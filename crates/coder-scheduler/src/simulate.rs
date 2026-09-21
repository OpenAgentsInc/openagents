//! The deterministic simulation: two refill policies, two lane models,
//! one clock.
//!
//! `run` drives a catalog through a discrete-event clock in stated
//! ticks and measures what the supervision styles do with it:
//!
//! - [`Fill::Waves`] — a wave of admissions runs to *its* end; freed
//!   slots wait for the slowest task in the wave before the next plan.
//!   This is the shape of a fan-out that returns the batch only when
//!   every task has settled.
//! - [`Fill::Refill`] — every completion replans immediately, so a freed
//!   slot takes the next ready task while its wave-mates still run.
//!
//! A second axis asks what the resource lanes are worth:
//!
//! - [`Lanes::Separate`] — every declared resource dimension bounds
//!   admission: executor slots, CPU units, memory, and the integration
//!   lane.
//! - [`Lanes::SessionCount`] — only executor slots bound admission, the
//!   baseline a session-count scheduler would see. CPU, memory, and
//!   integration admissions go unchecked, and the report's peaks show
//!   the contention that admits.
//!
//! Every combination schedules through the real [`crate::plan::select`],
//! so the simulation exercises the same dependencies, conflicts,
//! quiet-host lane, and resource bounds a live run obeys. The quiet-host
//! rule stays in force under both lane models — it is a scheduling rule,
//! not a quantity. Review is not modeled — an attempt that completes
//! here is instantly `completed` — because the question this answers is
//! how dispatch order moves the makespan, not how fast a reviewer reads.
//! Fields the simulation cannot observe — gate time, failures, retries,
//! spend — record [`Observed::Unknown`], never a fabricated zero.
//!
//! The numbers are simulated ticks over pinned estimates. They compare
//! the policies against each other on the same catalog and say nothing
//! about live wall-clock speed — no live-speedup claim is made or
//! licensed by this output.

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

/// Whether the host's resources bound admission as separate lanes or as
/// one aggregate session count.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lanes {
    /// Every declared dimension bounds admission — executor slots, CPU
    /// units, memory, and the integration lane.
    Separate,
    /// Only executor slots bound admission. CPU, memory, and
    /// integration requests go unchecked — the shape of a scheduler
    /// that counts sessions and nothing else.
    SessionCount,
}

impl Lanes {
    /// The lane model's name in reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Separate => "separate",
            Self::SessionCount => "session-count",
        }
    }
}

/// A count the run either measured or could not observe.
///
/// The simulation has no executor, no reviewer, and no price list, so
/// some acceptance fields are unobservable by construction. Those
/// record `Unknown` — serialized as the string `unknown` — rather than
/// a zero that would read as a measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Observed {
    /// The run measured the count.
    Known(u64),
    /// The simulation cannot observe the quantity.
    Unknown,
}

impl serde::Serialize for Observed {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Known(count) => serializer.serialize_u64(*count),
            Self::Unknown => serializer.serialize_str("unknown"),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Observed {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Count(u64),
            Tag(String),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Count(count) => Ok(Self::Known(count)),
            Repr::Tag(tag) if tag == "unknown" => Ok(Self::Unknown),
            Repr::Tag(tag) => Err(serde::de::Error::custom(format!(
                "an unobserved field is `unknown`, not `{tag}`"
            ))),
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
///
/// Peaks record what the admitted set actually held — under
/// [`Lanes::SessionCount`] a peak can exceed the host's declared total,
/// which is the contention the aggregate model admits.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Peak {
    /// The most executor places held at once.
    pub executor_slots: u32,
    /// The most CPU units held at once.
    pub cpu_units: u32,
    /// The most memory reserved at once, in MiB.
    pub memory_mib: u64,
    /// The most integration lanes held at once.
    pub integration_lanes: u32,
    /// Whether the quiet-host lane was ever held.
    pub quiet_host_held: bool,
}

/// What one simulated run produced.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    /// The refill policy.
    pub policy: Fill,
    /// The lane model.
    pub lanes: Lanes,
    /// The executor-slot capacity the run used.
    pub capacity_executor_slots: u32,
    /// The last completion tick — 0 when nothing scheduled.
    pub makespan_ticks: u64,
    /// How many waves the run took (one per plan round).
    pub rounds: u32,
    /// Tasks that ran.
    pub scheduled: u32,
    /// Tasks the run accepted. The simulation treats a completion as
    /// accepted because review is not modeled, so this is the scheduled
    /// count restated as the model's own verdict.
    pub accepted: Observed,
    /// Tasks that never ran, with the plan's last stated reasons.
    pub unscheduled: BTreeMap<String, Vec<String>>,
    /// Every completion, in finish order.
    pub completions: Vec<Completion>,
    /// Mean queue wait over the scheduled tasks, in ticks.
    pub mean_queue_wait_ticks: f64,
    /// The worst queue wait, in ticks.
    pub max_queue_wait_ticks: u64,
    /// Time spent in review or a verification gate — the simulation
    /// models neither, so this is `unknown`.
    pub gate_ticks: Observed,
    /// Attempts that failed — the simulation runs no executor, so no
    /// attempt can fail or be observed failing.
    pub failures: Observed,
    /// Attempts retried — with no failure path there is nothing to
    /// retry, and the field stays `unknown` rather than reading as a
    /// measured zero.
    pub retries: Observed,
    /// What the run cost — the simulation has no spend model.
    pub spend: Observed,
    /// The busiest the host became.
    pub peak: Peak,
}

/// Run one simulation under separate resource lanes.
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
    run_lanes(
        catalog,
        capacity,
        fill,
        Lanes::Separate,
        externally_completed,
    )
}

/// Run one simulation under a stated lane model.
///
/// [`Lanes::SessionCount`] relaxes every quantitative bound but
/// executor slots — the plan sees unbounded CPU, memory, and
/// integration, while the recorded peaks still show what the admitted
/// set actually held.
#[must_use]
pub fn run_lanes(
    catalog: &Catalog,
    capacity: &Capacity,
    fill: Fill,
    lanes: Lanes,
    externally_completed: &BTreeSet<String>,
) -> Report {
    let policy = Policy {
        // Review is not modeled; keep the intake open.
        review_cap: u32::MAX,
    };
    let effective = match lanes {
        Lanes::Separate => *capacity,
        Lanes::SessionCount => Capacity {
            executor_slots: capacity.executor_slots,
            cpu_units: u32::MAX,
            memory_mib: u64::MAX,
            integration_lanes: u32::MAX,
        },
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
            capacity: &effective,
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
            peak.integration_lanes = peak.integration_lanes.max(used.integration);
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
        lanes,
        capacity_executor_slots: capacity.executor_slots,
        makespan_ticks: completions
            .iter()
            .map(|completion| completion.end_tick)
            .max()
            .unwrap_or(0),
        rounds,
        scheduled,
        // Completion is acceptance here — review is not modeled.
        accepted: Observed::Known(u64::from(scheduled)),
        unscheduled,
        completions,
        mean_queue_wait_ticks: mean,
        max_queue_wait_ticks: waits.iter().copied().max().unwrap_or(0),
        gate_ticks: Observed::Unknown,
        failures: Observed::Unknown,
        retries: Observed::Unknown,
        spend: Observed::Unknown,
        peak,
    }
}

/// The pinned workloads the policy comparisons run over.
///
/// Each graph is a deterministic fixture — fixed ids, issues,
/// dependencies, footprints, and tick estimates, with no randomness and
/// no clock — so a report names its workload by the catalog's own
/// digest. `docs/coder/measurements/2026-09-21-wave-vs-refill.md`
/// records the digests and the numbers each produced.
pub mod fixtures {
    use std::collections::BTreeSet;

    use crate::catalog::{Catalog, Footprint, Task};
    use crate::resources::Resources;

    /// The dependency `mixed-dag` waits on that no catalog holds — it
    /// completes outside the run and arrives through
    /// `externally_completed`.
    pub const EXTERNAL_DEP: &str = "external:vendor-1";

    fn task(id: &str, issue: u64, ticks: u64, writes: &[&str]) -> Task {
        Task {
            id: id.to_string(),
            issue,
            base: "sha256:fixture-base".to_string(),
            input: format!("sha256:fixture-input-{id}"),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec![],
                writes: writes.iter().map(|path| path.to_string()).collect(),
            },
            priority: 0,
            resources: Resources::default(),
            estimate_ticks: ticks,
        }
    }

    fn sealed(name: &str, tasks: Vec<Task>) -> Catalog {
        let mut catalog = Catalog::new(name, tasks);
        catalog.seal().expect("a fixture catalog validates");
        catalog
    }

    /// `chain`: six tasks in one dependency chain — serial by
    /// construction, so the two refill policies must tie on it.
    pub fn chain() -> Catalog {
        let ticks = [2_u64, 3, 1, 4, 2, 3];
        let mut tasks = Vec::new();
        for (index, ticks) in ticks.iter().enumerate() {
            let id = format!("c{index}");
            let mut task = task(
                &id,
                index as u64 + 1,
                *ticks,
                &[&format!("crates/chain/{id}.rs")],
            );
            if index > 0 {
                task.depends_on = vec![format!("c{}", index - 1)];
            }
            tasks.push(task);
        }
        sealed("chain", tasks)
    }

    /// `fan-out`: a short root, eight skewed leaves, and a join — the
    /// shape where a slot freed early in a wave sits idle the longest.
    pub fn fan_out() -> Catalog {
        let mut tasks = vec![task("root", 1, 2, &["crates/fan/root.rs"])];
        let ticks = [8_u64, 1, 1, 1, 1, 1, 1, 1];
        for (index, ticks) in ticks.iter().enumerate() {
            let id = format!("w{index}");
            let mut task = task(
                &id,
                index as u64 + 2,
                *ticks,
                &[&format!("crates/fan/{id}.rs")],
            );
            task.depends_on = vec!["root".to_string()];
            tasks.push(task);
        }
        let mut join = task("join", 10, 1, &["docs/fan-out.md"]);
        join.depends_on = tasks[1..].iter().map(|task| task.id.clone()).collect();
        tasks.push(join);
        sealed("fan-out", tasks)
    }

    /// `mixed-dag`: the workload where the two axes interact — a
    /// dependency diamond, shared reads against live writes, a
    /// write-write conflict over one document, two integration-lane
    /// tasks, a heavyweight build, a quiet-host measurement that drains
    /// whatever is in flight, and one task gated on an externally
    /// completed dependency.
    pub fn mixed_dag() -> Catalog {
        let survey = task("survey", 1, 3, &["docs/survey.md"]);
        let mut core_a = task("core-a", 2, 4, &["crates/core/a.rs"]);
        core_a.depends_on = vec!["survey".to_string()];
        let mut core_b = task("core-b", 3, 6, &["crates/core/b.rs"]);
        core_b.depends_on = vec!["survey".to_string()];
        let mut join = task("join-report", 4, 2, &["docs/join-report.md"]);
        join.depends_on = vec!["core-a".to_string(), "core-b".to_string()];
        let mut reader = task("shared-reader", 5, 3, &["notes/reader.md"]);
        reader.footprint = Footprint::Declared {
            reads: vec!["crates/core".to_string()],
            writes: vec!["notes/reader.md".to_string()],
        };
        let docs_a = task("docs-a", 6, 2, &["docs/roll-up.md"]);
        let mut measure = task("measure", 7, 4, &["docs/measure.md"]);
        measure.depends_on = vec!["join-report".to_string()];
        measure.resources.quiet_host = true;
        let mut land_a = task("land-a", 8, 3, &["crates/land/a.rs"]);
        land_a.resources.integration = true;
        let mut land_b = task("land-b", 9, 2, &["crates/land/b.rs"]);
        land_b.resources.integration = true;
        let docs_b = task("docs-b", 10, 5, &["docs/roll-up.md"]);
        let mut build = task("heavy-build", 11, 7, &["target/heavy-build"]);
        build.resources.cpu_units = 8;
        build.resources.memory_mib = 8192;
        let mut vendor = task("upstream-fix", 12, 2, &["crates/vendor/fix.rs"]);
        vendor.depends_on = vec![EXTERNAL_DEP.to_string()];
        sealed(
            "mixed-dag",
            vec![
                survey, core_a, core_b, join, reader, docs_a, measure, land_a, land_b, docs_b,
                build, vendor,
            ],
        )
    }

    /// The completions `mixed-dag`'s external dependency arrives
    /// through. A run that does not supply them leaves `upstream-fix`
    /// unscheduled — absent is never assumed complete.
    pub fn mixed_dag_externals() -> BTreeSet<String> {
        BTreeSet::from([EXTERNAL_DEP.to_string()])
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

    #[test]
    fn each_fixture_produces_a_deterministic_report() {
        let externals = fixtures::mixed_dag_externals();
        for (catalog, externals) in [
            (fixtures::chain(), &BTreeSet::new()),
            (fixtures::fan_out(), &BTreeSet::new()),
            (fixtures::mixed_dag(), &externals),
        ] {
            for fill in [Fill::Waves, Fill::Refill] {
                for lanes in [Lanes::Separate, Lanes::SessionCount] {
                    let first = run_lanes(&catalog, &host(4), fill, lanes, externals);
                    let second = run_lanes(&catalog, &host(4), fill, lanes, externals);
                    assert_eq!(
                        serde_json::to_string(&first).unwrap(),
                        serde_json::to_string(&second).unwrap(),
                        "{} under {}/{}",
                        catalog.name,
                        fill.name(),
                        lanes.name()
                    );
                }
            }
        }
    }

    /// The digests the measurement doc cites — a fixture edit repins
    /// its workload and must re-measure, not quietly inherit the old
    /// numbers.
    #[test]
    fn the_fixture_digests_are_pinned() {
        assert_eq!(
            fixtures::chain().digest,
            "sha256:5554ed8cd76d7e93db32ec7c570636d6b6157596c277861548a06b7db4c6d636"
        );
        assert_eq!(
            fixtures::fan_out().digest,
            "sha256:e7bc1993cb56287d94e234f389043ee48bf443fd16d562e0a76975fbb18aa47c"
        );
        assert_eq!(
            fixtures::mixed_dag().digest,
            "sha256:2d375c22d29b55ee68167a4aad6326856b989e484ec2322584aaa88e881b79ff"
        );
    }

    #[test]
    fn a_hand_computed_graph_reports_exact_ticks() {
        // a(2) and b(5) are independent; c(3) waits on a.
        let mut c = task("c", 3);
        c.depends_on = vec!["a".to_string()];
        let catalog = catalog(vec![task("a", 2), task("b", 5), c]);

        // Two slots: a and b start at 0; c starts the tick a finishes.
        let report = run(&catalog, &host(2), Fill::Refill, &BTreeSet::new());
        assert_eq!(report.makespan_ticks, 5);
        assert_eq!(report.scheduled, 3);
        for completion in &report.completions {
            assert_eq!(completion.queue_wait_ticks, 0);
        }

        // One slot: the stated order serializes as a 0→2, b 2→7, and c
        // — ready since 2 — waits to 7 and ends at 10.
        let report = run(&catalog, &host(1), Fill::Refill, &BTreeSet::new());
        assert_eq!(report.makespan_ticks, 10);
        let c = report
            .completions
            .iter()
            .find(|completion| completion.task == "c")
            .unwrap();
        assert_eq!(c.ready_tick, 2);
        assert_eq!(c.start_tick, 7);
        assert_eq!(c.queue_wait_ticks, 5);
        assert_eq!(report.max_queue_wait_ticks, 5);
        assert!((report.mean_queue_wait_ticks - 7.0 / 3.0).abs() < f64::EPSILON);

        // Waves take the same path — on a serialized host the policies
        // tie rather than trade.
        let waves = run(&catalog, &host(1), Fill::Waves, &BTreeSet::new());
        assert_eq!(waves.makespan_ticks, 10);
    }

    #[test]
    fn fields_the_simulation_cannot_observe_report_unknown() {
        let catalog = catalog(vec![task("a", 1)]);
        for fill in [Fill::Waves, Fill::Refill] {
            let report = run(&catalog, &host(1), fill, &BTreeSet::new());
            assert_eq!(report.accepted, Observed::Known(1));
            assert_eq!(report.gate_ticks, Observed::Unknown);
            assert_eq!(report.failures, Observed::Unknown);
            assert_eq!(report.retries, Observed::Unknown);
            assert_eq!(report.spend, Observed::Unknown);
        }
        let report = run(&catalog, &host(1), Fill::Refill, &BTreeSet::new());
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["accepted"], 1);
        assert_eq!(json["gate_ticks"], "unknown");
        assert_eq!(json["failures"], "unknown");
        assert_eq!(json["retries"], "unknown");
        assert_eq!(json["spend"], "unknown");
    }

    #[test]
    fn session_count_lanes_oversubscribe_what_separate_lanes_bound() {
        // Three 4-GiB tasks on an 8-GiB host: separate lanes admit two
        // at a time; a session count admits all three and oversubscribes
        // memory by half.
        let tasks = ["m1", "m2", "m3"].map(|id| {
            let mut task = task(id, 4);
            task.resources.memory_mib = 4096;
            task
        });
        let catalog = catalog(tasks.into());
        let host = Capacity {
            executor_slots: 4,
            cpu_units: 64,
            memory_mib: 8192,
            integration_lanes: 1,
        };
        let separate = run_lanes(
            &catalog,
            &host,
            Fill::Refill,
            Lanes::Separate,
            &BTreeSet::new(),
        );
        let counted = run_lanes(
            &catalog,
            &host,
            Fill::Refill,
            Lanes::SessionCount,
            &BTreeSet::new(),
        );
        assert_eq!(separate.peak.memory_mib, 8192);
        assert_eq!(separate.makespan_ticks, 8, "two waves' worth of memory");
        assert_eq!(counted.peak.memory_mib, 12288);
        assert_eq!(counted.makespan_ticks, 4);
    }

    #[test]
    fn session_count_lanes_run_integrations_concurrently() {
        let externals = fixtures::mixed_dag_externals();
        let catalog = fixtures::mixed_dag();
        let separate = run_lanes(&catalog, &host(4), Fill::Refill, Lanes::Separate, &externals);
        let counted = run_lanes(
            &catalog,
            &host(4),
            Fill::Refill,
            Lanes::SessionCount,
            &externals,
        );
        assert_eq!(separate.peak.integration_lanes, 1);
        assert_eq!(counted.peak.integration_lanes, 2);
    }

    #[test]
    fn the_mixed_dag_holds_its_ordering_rules_under_every_policy() {
        let externals = fixtures::mixed_dag_externals();
        let catalog = fixtures::mixed_dag();
        for fill in [Fill::Waves, Fill::Refill] {
            for lanes in [Lanes::Separate, Lanes::SessionCount] {
                let report = run_lanes(&catalog, &host(4), fill, lanes, &externals);
                assert_eq!(report.scheduled, 12, "{}/{}", fill.name(), lanes.name());
                assert!(report.unscheduled.is_empty());
                let interval = |id: &str| {
                    let completion = report
                        .completions
                        .iter()
                        .find(|completion| completion.task == id)
                        .unwrap_or_else(|| panic!("{id} scheduled"));
                    (completion.start_tick, completion.end_tick)
                };
                let overlaps =
                    |a: (u64, u64), b: (u64, u64)| a.0 < b.1 && b.0 < a.1;
                // The write-write pair never overlaps.
                assert!(!overlaps(interval("docs-a"), interval("docs-b")));
                // The quiet measurement runs alone.
                let quiet = interval("measure");
                for completion in &report.completions {
                    if completion.task == "measure" {
                        continue;
                    }
                    assert!(
                        completion.end_tick <= quiet.0 || completion.start_tick >= quiet.1,
                        "{} overlaps the quiet lane under {}/{}",
                        completion.task,
                        fill.name(),
                        lanes.name()
                    );
                }
                // Dependencies hold: the diamond joins after both cores.
                assert!(interval("join-report").0 >= interval("core-a").1);
                assert!(interval("join-report").0 >= interval("core-b").1);
            }
        }
    }

    #[test]
    fn an_unsupplied_external_dependency_stays_unscheduled() {
        let catalog = fixtures::mixed_dag();
        let report = run(&catalog, &host(4), Fill::Refill, &BTreeSet::new());
        assert_eq!(report.scheduled, 11);
        assert!(report.unscheduled.contains_key("upstream-fix"));
        assert!(
            report.unscheduled["upstream-fix"]
                .iter()
                .any(|reason| reason.contains("unknown"))
        );
        let supplied = run(
            &catalog,
            &host(4),
            Fill::Refill,
            &fixtures::mixed_dag_externals(),
        );
        assert!(supplied.unscheduled.is_empty());
        assert_eq!(supplied.scheduled, 12);
    }

    /// Prints the benchmark table recorded in
    /// `docs/coder/measurements/2026-09-21-wave-vs-refill.md`. Run with
    /// `cargo test -p coder-scheduler print_fixture_benchmark -- --ignored`.
    #[test]
    #[ignore = "prints the measurement table"]
    fn print_fixture_benchmark() {
        let externals = fixtures::mixed_dag_externals();
        for (catalog, externals) in [
            (fixtures::chain(), &BTreeSet::new()),
            (fixtures::fan_out(), &BTreeSet::new()),
            (fixtures::mixed_dag(), &externals),
        ] {
            println!("{} {}", catalog.name, catalog.digest);
            for slots in [2_u32, 4, 8] {
                let host = host(slots);
                for fill in [Fill::Waves, Fill::Refill] {
                    for lanes in [Lanes::Separate, Lanes::SessionCount] {
                        let report = run_lanes(&catalog, &host, fill, lanes, externals);
                        println!(
                            "  slots={slots} {:6} {:13} makespan={:3} rounds={:2} \
                             scheduled={:2} unscheduled={} mean-wait={:.2} max-wait={} \
                             peak-slots={} peak-cpu={} peak-mib={} peak-int={} quiet={}",
                            fill.name(),
                            lanes.name(),
                            report.makespan_ticks,
                            report.rounds,
                            report.scheduled,
                            report.unscheduled.len(),
                            report.mean_queue_wait_ticks,
                            report.max_queue_wait_ticks,
                            report.peak.executor_slots,
                            report.peak.cpu_units,
                            report.peak.memory_mib,
                            report.peak.integration_lanes,
                            report.peak.quiet_host_held,
                        );
                    }
                }
            }
        }
    }
}
