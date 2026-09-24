//! The admission plan: a pure function, a stable order, a stated reason.
//!
//! `select` answers *which queued tasks may start now*. It is
//! deterministic — the same catalog, states, capacity, exclusions, and
//! policy always produce the same plan — and it is explanatory: every
//! task it does not admit comes back with the reasons why, so a blocked
//! backlog reads as a list of causes rather than a silence.
//!
//! # Ordering and fairness
//!
//! Candidates run in one stated order: priority descending, then issue
//! number ascending, then id ascending. The order never depends on the
//! catalog's listing order, a hash map's iteration, or the time of day,
//! so two runs of the same inputs admit the same tasks in the same
//! order. Admission is greedy in that order: a higher-priority task that
//! cannot fit does not starve a lower-priority task that can, and a task
//! that loses this round keeps its place — its reasons are recorded, and
//! the next plan under freed capacity reconsiders it before any task
//! that arrives behind it.
//!
//! # The quiet-host lane
//!
//! A task marked `quiet_host` admits only onto an empty host: no active
//! work, nothing admitted earlier in the same round. And the lane is
//! *held* — while a quiet task runs, every later plan refuses all other
//! admissions, including work submitted afterward. To keep a ready quiet
//! task from starving behind an endless refill, a ready quiet task also
//! *drains* the host: while one waits, no new ordinary work admits, so
//! the in-flight set empties and the quiet task goes next.
//!
//! # What counts as occupying the host
//!
//! - `active` tasks hold their full resource vector and their full
//!   footprint.
//! - `review` tasks hold their write footprint — the landed result is
//!   still under independent review, so another writer over the same
//!   paths is refused — but no executor resources, and they count
//!   against the review cap.
//! - `unknown` tasks — a crashed attempt, recovered conservatively —
//!   hold everything: resources and footprint, because the attempt may
//!   still be running somewhere the ledger cannot see.
//! - `completed` and `rejected` tasks hold nothing.
//!
//! # Backpressure
//!
//! Results await review before acceptance, and review is human. When the
//! number of tasks awaiting review reaches the policy's stated cap,
//! admissions stop — the scheduler slows the intake rather than pile
//! unverified results deeper.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::catalog::{Catalog, ConflictKind, Footprint, footprints_conflict};
use crate::resources::{Bound, Capacity, InUse};

/// The scheduler's durable view of one task.
///
/// The ledger keeps the full record; the plan needs only where the task
/// stands. Tasks absent from the states map are `Queued`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// In the catalog, never dispatched or returned to the queue.
    #[default]
    Queued,
    /// An attempt is in flight.
    Active,
    /// The attempt returned a result that awaits independent review.
    Review,
    /// A result was verified and accepted. Terminal — the only state a
    /// dependency reads as done.
    Completed,
    /// A result was reviewed and refused. The task waits for an explicit
    /// requeue.
    Rejected,
    /// The writer crashed with the attempt in flight. Conservative: the
    /// attempt's outcome cannot be stated, so the task stays blocked and
    /// holds its footprint until it is explicitly reconciled.
    Unknown,
}

/// The stated bounds on one plan round.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Policy {
    /// The most tasks allowed to await review before admissions stop —
    /// the backpressure bound on the review backlog.
    pub review_cap: u32,
}

/// The path-owner check's external side: a footprint some work outside
/// this scheduler owns, treated as writes the scheduler cannot see.
///
/// A candidate conflicts with an exclusion when anything it reads or
/// writes overlaps the owned path — an external owner may rewrite its
/// paths at any time, so even a shared-looking read is a race the plan
/// refuses.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Exclusion {
    /// A name for the owner, for explanations — a run, an operator, an
    /// integration in flight.
    pub owner: String,
    /// The owned paths, validated like a task's footprint paths.
    pub writes: Vec<String>,
}

/// Everything one plan round reads.
pub struct Input<'a> {
    /// The pinned backlog.
    pub catalog: &'a Catalog,
    /// What the host offers.
    pub capacity: &'a Capacity,
    /// Durable status per task id; absent means `Queued`.
    pub states: &'a BTreeMap<String, Status>,
    /// Task ids completed outside this run. A dependency absent from the
    /// catalog resolves only here — everywhere else it is `unknown`.
    pub externally_completed: &'a BTreeSet<String>,
    /// Paths owned by work outside this scheduler.
    pub exclusions: &'a [Exclusion],
    /// The round's bounds.
    pub policy: &'a Policy,
}

/// One admitted task and the footprint/resources it now holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Admission {
    /// The admitted task's stable id.
    pub task: String,
    /// The task's place in the round's stated order.
    pub order: u32,
}

/// A task the plan did not admit, and why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Blocked {
    /// The task's stable id.
    pub task: String,
    /// Every reason that applies — a task can be short on capacity and
    /// colliding at once, and the plan says both.
    pub reasons: Vec<Reason>,
}

/// Why a task did not admit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reason {
    /// A dependency is in the catalog but not yet complete — its own
    /// state is named.
    Dependency { dependency: String, state: Status },
    /// A dependency is in no catalog and no external completion set —
    /// its state is `unknown`, and absent is never complete.
    DependencyAbsent { dependency: String },
    /// The task's footprint collides with another running or admitted
    /// task's. The colliding paths are named.
    Conflict {
        /// The task holding the other side.
        other: String,
        /// This task's colliding path.
        path: String,
        /// The other side's colliding path, when it named one.
        other_path: Option<String>,
        /// Write-write or write-read.
        kind: ConflictKind,
    },
    /// The task's footprint reaches a path an external owner holds.
    Exclusion { owner: String, path: String },
    /// The candidate needs the quiet-host lane and active work occupies
    /// the host — the quiet task waits for the host to empty.
    QuietHostBusy,
    /// A quiet-host task holds the host — the named task — and excludes
    /// all other admissions while it runs, including later work.
    QuietHostHeld { holder: String },
    /// A quiet-host task is ready and waiting — the host drains, and no
    /// new ordinary work admits until it has run.
    QuietDrain { waiting: String },
    /// The resource bound the admission would exceed.
    Capacity(Bound),
    /// The review backlog reached its stated cap.
    ReviewBacklog { cap: u32 },
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency { dependency, state } => {
                write!(
                    f,
                    "dependency `{dependency}` is not completed yet (its state is {state:?})"
                )
            }
            Self::DependencyAbsent { dependency } => write!(
                f,
                "dependency `{dependency}` is not in the catalog or the list of completed tasks, so its state is unknown"
            ),
            Self::Conflict {
                other,
                path,
                other_path,
                kind,
            } => {
                let other_path = other_path.as_deref().map_or_else(
                    || "paths it did not list".to_string(),
                    |path| format!("`{path}`"),
                );
                match kind {
                    ConflictKind::WriteWrite => write!(
                        f,
                        "both this task and `{other}` write files: `{path}` overlaps {other_path}"
                    ),
                    ConflictKind::WriteRead => write!(
                        f,
                        "one of this task and `{other}` writes files the other reads: \
                         `{path}` overlaps {other_path}"
                    ),
                }
            }
            Self::Exclusion { owner, path } => {
                write!(
                    f,
                    "path `{path}` belongs to `{owner}`, outside this scheduler"
                )
            }
            Self::QuietHostBusy => {
                write!(
                    f,
                    "the task needs the host to itself, and other tasks are running"
                )
            }
            Self::QuietHostHeld { holder } => write!(
                f,
                "task `{holder}` needs the host to itself and is running, so no other task can start"
            ),
            Self::QuietDrain { waiting } => write!(
                f,
                "task `{waiting}` needs the host to itself and is waiting, so no new task starts until the running ones finish"
            ),
            Self::Capacity(bound) => {
                let resource = match bound {
                    Bound::ExecutorSlots => "enough free executor slots",
                    Bound::CpuUnits => "enough free CPU units",
                    Bound::MemoryMib => "enough free memory",
                    Bound::Integration => "a free integration lane",
                };
                write!(f, "the host does not have {resource} for this task")
            }
            Self::ReviewBacklog { cap } => write!(
                f,
                "the review queue is full: {cap} of {cap} finished tasks are waiting for review; review some before more tasks start"
            ),
        }
    }
}

/// One round's answer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Plan {
    /// The admissions, in the round's stated order.
    pub admit: Vec<Admission>,
    /// The tasks not admitted, with their reasons.
    pub blocked: Vec<Blocked>,
    /// The tasks already occupying the host — active, in review, or
    /// unknown — for the record.
    pub occupying: Vec<String>,
}

/// Plan one round.
///
/// Reads the durable states, the capacity, and the exclusions; writes
/// nothing. The caller dispatches `admit` itself and records the claims.
#[must_use]
pub fn select(input: &Input<'_>) -> Plan {
    let by_id: BTreeMap<&str, &crate::catalog::Task> = input
        .catalog
        .tasks
        .iter()
        .map(|task| (task.id.as_str(), task))
        .collect();

    // What occupies the host, which footprints are still claimed, and
    // who holds the quiet lane.
    let mut used = InUse::default();
    let mut held: Vec<(&str, &Footprint)> = Vec::new();
    let mut occupying = Vec::new();
    let mut active: u32 = 0;
    let mut quiet_holder: Option<String> = None;
    let mut reviewing: u32 = 0;
    for task in &input.catalog.tasks {
        let state = input
            .states
            .get(task.id.as_str())
            .copied()
            .unwrap_or_default();
        match state {
            Status::Active | Status::Unknown => {
                // Active work holds what it declared. An unknown attempt
                // is held the same way — it may still be running
                // somewhere the ledger cannot see.
                used.add(&task.resources);
                held.push((task.id.as_str(), &task.footprint));
                occupying.push(task.id.clone());
                active += 1;
                if task.resources.quiet_host {
                    quiet_holder = Some(task.id.clone());
                }
            }
            Status::Review => {
                // The result awaits review; its writes stay claimed, its
                // executor resources do not.
                held.push((task.id.as_str(), &task.footprint));
                occupying.push(task.id.clone());
                reviewing += 1;
            }
            Status::Queued | Status::Completed | Status::Rejected => {}
        }
    }

    // Candidates in the stated order: priority descending, issue number
    // ascending, id ascending.
    let mut candidates: Vec<&crate::catalog::Task> = input
        .catalog
        .tasks
        .iter()
        .filter(|task| {
            input
                .states
                .get(task.id.as_str())
                .copied()
                .unwrap_or_default()
                == Status::Queued
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(a.issue.cmp(&b.issue))
            .then(a.id.cmp(&b.id))
    });

    // A ready quiet task drains the host: find the first one before the
    // round begins, so every ordinary candidate sees it. *Ready* means
    // its only impediment is occupancy — dependencies met, no exclusion
    // hit, and able to fit an empty host. A quiet task that can never
    // run drains nothing.
    let quiet_waiting = candidates
        .iter()
        .filter(|task| task.resources.quiet_host)
        .find(|task| quiet_ready(task, input, &by_id))
        .map(|task| task.id.clone());

    let backlog_full = reviewing >= input.policy.review_cap;

    let mut plan = Plan {
        occupying,
        ..Plan::default()
    };
    let mut order = 0_u32;
    for task in candidates {
        let mut reasons = dependencies_met(task, input, &by_id);
        if backlog_full {
            reasons.push(Reason::ReviewBacklog {
                cap: input.policy.review_cap,
            });
        }
        if let Some(holder) = &quiet_holder {
            // A quiet task holds the host — every later admission is
            // refused, including work submitted after the quiet task.
            reasons.push(Reason::QuietHostHeld {
                holder: holder.clone(),
            });
        } else if task.resources.quiet_host {
            if active > 0 {
                reasons.push(Reason::QuietHostBusy);
            }
        } else if let Some(waiting) = &quiet_waiting {
            reasons.push(Reason::QuietDrain {
                waiting: waiting.clone(),
            });
        }
        for exclusion in input.exclusions {
            if let Some(conflict) = footprint_vs_paths(&task.footprint, &exclusion.writes) {
                reasons.push(Reason::Exclusion {
                    owner: exclusion.owner.clone(),
                    path: conflict,
                });
            }
        }
        for (other, footprint) in &held {
            if let Some(conflict) = footprints_conflict(&task.footprint, footprint) {
                reasons.push(Reason::Conflict {
                    other: (*other).to_string(),
                    path: conflict.path_a,
                    other_path: conflict.path_b,
                    kind: conflict.kind,
                });
            }
        }
        if let Some(bound) = used.exceeds(input.capacity, &task.resources) {
            reasons.push(Reason::Capacity(bound));
        }
        if reasons.is_empty() {
            order += 1;
            plan.admit.push(Admission {
                task: task.id.clone(),
                order,
            });
            used.add(&task.resources);
            held.push((task.id.as_str(), &task.footprint));
            active += 1;
            if task.resources.quiet_host {
                quiet_holder = Some(task.id.clone());
            }
        } else {
            plan.blocked.push(Blocked {
                task: task.id.clone(),
                reasons,
            });
        }
    }
    plan
}

/// A candidate's dependency reasons — empty when every dependency is
/// complete.
fn dependencies_met(
    task: &crate::catalog::Task,
    input: &Input<'_>,
    by_id: &BTreeMap<&str, &crate::catalog::Task>,
) -> Vec<Reason> {
    let mut reasons = Vec::new();
    for dependency in &task.depends_on {
        if by_id.contains_key(dependency.as_str()) {
            let state = input.states.get(dependency).copied().unwrap_or_default();
            if state != Status::Completed {
                reasons.push(Reason::Dependency {
                    dependency: dependency.clone(),
                    state,
                });
            }
        } else if !input.externally_completed.contains(dependency) {
            reasons.push(Reason::DependencyAbsent {
                dependency: dependency.clone(),
            });
        }
    }
    reasons
}

/// Is a quiet candidate's only impediment the host's occupancy —
/// dependencies met, no exclusion conflict, and able to fit an empty
/// host? Footprint conflicts with active work do not count: draining
/// clears exactly those.
fn quiet_ready(
    task: &crate::catalog::Task,
    input: &Input<'_>,
    by_id: &BTreeMap<&str, &crate::catalog::Task>,
) -> bool {
    dependencies_met(task, input, by_id).is_empty()
        && input
            .exclusions
            .iter()
            .all(|exclusion| footprint_vs_paths(&task.footprint, &exclusion.writes).is_none())
        && InUse::default()
            .exceeds(input.capacity, &task.resources)
            .is_none()
}

/// Does the footprint reach any externally owned path — reads and
/// writes both, because an owner may rewrite its paths at will.
fn footprint_vs_paths(footprint: &Footprint, owned: &[String]) -> Option<String> {
    match footprint {
        Footprint::Unknown => owned.first().cloned().or(Some("<unknown>".to_string())),
        Footprint::Declared { reads, writes } => {
            for path in writes.iter().chain(reads.iter()) {
                for other in owned {
                    if crate::catalog::paths_overlap(path, other) {
                        return Some(path.clone());
                    }
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Footprint, Task};
    use crate::resources::Resources;

    fn task(id: &str, issue: u64) -> Task {
        Task {
            id: id.to_string(),
            issue,
            base: "sha256:base".to_string(),
            input: format!("sha256:input-{id}"),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec![],
                writes: vec![format!("crates/{id}/src/lib.rs")],
            },
            priority: 0,
            resources: Resources::default(),
            estimate_ticks: 1,
        }
    }

    fn mk_catalog(tasks: Vec<Task>) -> Catalog {
        let mut catalog = Catalog::new("test", tasks);
        catalog.seal().unwrap();
        catalog
    }

    fn input<'a>(
        catalog: &'a Catalog,
        capacity: &'a Capacity,
        states: &'a BTreeMap<String, Status>,
        done: &'a BTreeSet<String>,
        exclusions: &'a [Exclusion],
        policy: &'a Policy,
    ) -> Input<'a> {
        Input {
            catalog,
            capacity,
            states,
            externally_completed: done,
            exclusions,
            policy,
        }
    }

    fn wide() -> Capacity {
        Capacity {
            executor_slots: 8,
            cpu_units: 64,
            memory_mib: 65536,
            integration_lanes: 1,
        }
    }

    fn policy() -> Policy {
        Policy { review_cap: 4 }
    }

    #[test]
    fn ready_tasks_admit_in_the_stated_order() {
        let mut a = task("a", 3);
        a.priority = 0;
        let mut b = task("b", 1);
        b.priority = 0;
        let mut c = task("c", 2);
        c.priority = 5;
        // The catalog's listing order must not matter.
        let catalog = mk_catalog(vec![a, c.clone(), b]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        let admitted: Vec<&str> = plan.admit.iter().map(|a| a.task.as_str()).collect();
        assert_eq!(admitted, ["c", "b", "a"], "priority, then issue, then id");
    }

    #[test]
    fn a_slot_bound_leaves_the_rest_blocked_with_reasons() {
        let catalog = mk_catalog(vec![task("a", 1), task("b", 2), task("c", 3)]);
        let capacity = Capacity {
            executor_slots: 1,
            ..wide()
        };
        let plan = select(&input(
            &catalog,
            &capacity,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert_eq!(plan.admit.len(), 1);
        assert_eq!(plan.admit[0].task, "a");
        for blocked in &plan.blocked {
            assert!(
                blocked
                    .reasons
                    .contains(&Reason::Capacity(Bound::ExecutorSlots))
            );
        }
    }

    #[test]
    fn an_absent_dependency_is_unknown_never_complete() {
        let mut t = task("a", 1);
        t.depends_on = vec!["ghost".to_string()];
        let catalog = mk_catalog(vec![t]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert!(plan.admit.is_empty());
        assert_eq!(
            plan.blocked[0].reasons,
            vec![Reason::DependencyAbsent {
                dependency: "ghost".to_string()
            }]
        );
        // An externally completed dependency resolves.
        let done = BTreeSet::from(["ghost".to_string()]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &done,
            &[],
            &policy(),
        ));
        assert_eq!(plan.admit.len(), 1);
    }

    #[test]
    fn a_dependency_waits_for_completed_not_dispatched() {
        let mut b = task("b", 2);
        b.depends_on = vec!["a".to_string()];
        let catalog = mk_catalog(vec![task("a", 1), b]);
        for state in [
            Status::Queued,
            Status::Active,
            Status::Review,
            Status::Rejected,
            Status::Unknown,
        ] {
            let states = BTreeMap::from([("a".to_string(), state)]);
            let plan = select(&input(
                &catalog,
                &wide(),
                &states,
                &BTreeSet::new(),
                &[],
                &policy(),
            ));
            assert!(
                plan.admit.iter().all(|admitted| admitted.task != "b"),
                "dependency in {state:?} does not release `b`"
            );
        }
        let states = BTreeMap::from([("a".to_string(), Status::Completed)]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &states,
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert!(plan.admit.iter().any(|admitted| admitted.task == "b"));
    }

    #[test]
    fn colliding_writes_serialize() {
        let mut a = task("a", 1);
        a.footprint = Footprint::Declared {
            reads: vec![],
            writes: vec!["docs".to_string()],
        };
        let mut b = task("b", 2);
        b.footprint = Footprint::Declared {
            reads: vec![],
            writes: vec!["docs/coder/runtime/terminal.md".to_string()],
        };
        let catalog = mk_catalog(vec![a, b]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert_eq!(plan.admit.len(), 1, "the ancestor write wins the round");
        assert!(matches!(
            plan.blocked[0].reasons[0],
            Reason::Conflict {
                kind: ConflictKind::WriteWrite,
                ..
            }
        ));
    }

    #[test]
    fn a_write_against_an_active_read_serializes() {
        let mut reader = task("reader", 1);
        reader.footprint = Footprint::Declared {
            reads: vec!["crates/jev/src/lib.rs".to_string()],
            writes: vec!["crates/reader/out.rs".to_string()],
        };
        let mut writer = task("writer", 2);
        writer.footprint = Footprint::Declared {
            reads: vec![],
            writes: vec!["crates/jev/src".to_string()],
        };
        let catalog = mk_catalog(vec![reader, writer]);
        let states = BTreeMap::from([("reader".to_string(), Status::Active)]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &states,
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert!(plan.admit.is_empty());
        assert!(matches!(
            plan.blocked[0].reasons[0],
            Reason::Conflict {
                kind: ConflictKind::WriteRead,
                ..
            }
        ));
    }

    #[test]
    fn shared_reads_admit_together() {
        let mut a = task("a", 1);
        a.footprint = Footprint::Declared {
            reads: vec!["Cargo.toml".to_string()],
            writes: vec!["crates/a/x.rs".to_string()],
        };
        let mut b = task("b", 2);
        b.footprint = Footprint::Declared {
            reads: vec!["Cargo.toml".to_string()],
            writes: vec!["crates/b/y.rs".to_string()],
        };
        let catalog = mk_catalog(vec![a, b]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert_eq!(plan.admit.len(), 2);
    }

    #[test]
    fn a_quiet_task_owns_the_host_and_drains_it() {
        let mut quiet = task("quiet", 2);
        quiet.resources.quiet_host = true;
        let ordinary = task("ordinary", 1);
        let catalog = mk_catalog(vec![ordinary, quiet]);

        // Nothing running: the quiet task admits alone and the ordinary
        // task is told the host drains.
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert_eq!(plan.admit.len(), 1);
        assert_eq!(plan.admit[0].task, "quiet");
        assert!(matches!(
            plan.blocked[0].reasons[0],
            Reason::QuietDrain { .. }
        ));

        // Work in flight: the quiet task waits for an empty host, and
        // the ordinary task still drains.
        let states = BTreeMap::from([("busy".to_string(), Status::Active)]);
        let mut busy = task("busy", 0);
        busy.resources.cpu_units = 1;
        let catalog = mk_catalog(vec![busy, task("ordinary", 1), {
            let mut q = task("quiet", 2);
            q.resources.quiet_host = true;
            q
        }]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &states,
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert!(plan.admit.is_empty());
        let quiet_reasons = &plan
            .blocked
            .iter()
            .find(|b| b.task == "quiet")
            .unwrap()
            .reasons;
        assert!(quiet_reasons.contains(&Reason::QuietHostBusy));
    }

    #[test]
    fn quiet_work_excludes_work_admitted_later() {
        let mut quiet = task("quiet", 1);
        quiet.resources.quiet_host = true;
        let catalog = mk_catalog(vec![quiet, task("ordinary", 2), task("later", 3)]);
        let states = BTreeMap::from([("quiet".to_string(), Status::Active)]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &states,
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert!(plan.admit.is_empty(), "nothing joins a quiet host");
        for blocked in &plan.blocked {
            assert!(
                blocked.reasons.contains(&Reason::QuietHostHeld {
                    holder: "quiet".to_string()
                }),
                "{blocked:?}"
            );
        }
    }

    #[test]
    fn a_permanently_blocked_quiet_task_drains_nothing() {
        // The quiet task can never run — it reaches an excluded path —
        // so it must not freeze the host behind it.
        let mut quiet = task("quiet", 1);
        quiet.resources.quiet_host = true;
        quiet.footprint = Footprint::Declared {
            reads: vec![],
            writes: vec!["owned/dir".to_string()],
        };
        let catalog = mk_catalog(vec![quiet, task("ordinary", 2)]);
        let exclusions = vec![Exclusion {
            owner: "root".to_string(),
            writes: vec!["owned".to_string()],
        }];
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &exclusions,
            &policy(),
        ));
        assert_eq!(plan.admit.len(), 1);
        assert_eq!(plan.admit[0].task, "ordinary");
        let quiet = plan.blocked.iter().find(|b| b.task == "quiet").unwrap();
        assert!(
            quiet
                .reasons
                .iter()
                .any(|reason| matches!(reason, Reason::Exclusion { .. }))
        );
    }

    #[test]
    fn an_excluded_path_is_refused_for_reads_and_writes() {
        let mut t = task("a", 1);
        t.footprint = Footprint::Declared {
            reads: vec!["Cargo.lock".to_string()],
            writes: vec!["crates/a/x.rs".to_string()],
        };
        let catalog = mk_catalog(vec![t]);
        let exclusions = vec![Exclusion {
            owner: "root-integration".to_string(),
            writes: vec!["Cargo.lock".to_string()],
        }];
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &exclusions,
            &policy(),
        ));
        assert!(plan.admit.is_empty());
        assert!(matches!(
            plan.blocked[0].reasons[0],
            Reason::Exclusion { .. }
        ));
    }

    #[test]
    fn the_review_cap_stops_the_intake() {
        let catalog = mk_catalog(vec![
            task("r1", 10),
            task("r2", 11),
            task("a", 1),
            task("b", 2),
        ]);
        let states = BTreeMap::from([
            ("r1".to_string(), Status::Review),
            ("r2".to_string(), Status::Review),
        ]);
        let policy = Policy { review_cap: 2 };
        let plan = select(&input(
            &catalog,
            &wide(),
            &states,
            &BTreeSet::new(),
            &[],
            &policy,
        ));
        assert!(plan.admit.is_empty());
        assert!(
            plan.blocked
                .iter()
                .all(|b| b.reasons.contains(&Reason::ReviewBacklog { cap: 2 }))
        );
    }

    #[test]
    fn an_unknown_attempt_holds_its_footprint() {
        let mut a = task("a", 1);
        a.footprint = Footprint::Declared {
            reads: vec![],
            writes: vec!["crates/shared".to_string()],
        };
        let mut b = task("b", 2);
        b.footprint = Footprint::Declared {
            reads: vec![],
            writes: vec!["crates/shared/x.rs".to_string()],
        };
        let catalog = mk_catalog(vec![a, b]);
        let states = BTreeMap::from([("a".to_string(), Status::Unknown)]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &states,
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert!(
            plan.admit.is_empty(),
            "an unknown attempt may still be writing"
        );
    }

    #[test]
    fn an_unknown_footprint_empties_the_host() {
        let mut a = task("a", 1);
        a.footprint = Footprint::Unknown;
        let b = task("b", 2);
        let catalog = mk_catalog(vec![b, a]);
        // The unknown footprint admits alone — onto an empty host it
        // collides with nothing, and it excludes everything behind it.
        let plan = select(&input(
            &catalog,
            &wide(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert_eq!(plan.admit.len(), 1);
        assert_eq!(plan.admit[0].task, "a");
        assert_eq!(plan.blocked[0].task, "b");
        assert!(matches!(
            plan.blocked[0].reasons[0],
            Reason::Conflict { .. }
        ));
        // And with work already running, it cannot join at all.
        let states = BTreeMap::from([("b".to_string(), Status::Active)]);
        let plan = select(&input(
            &catalog,
            &wide(),
            &states,
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert!(plan.admit.is_empty());
        assert!(matches!(
            plan.blocked[0].reasons[0],
            Reason::Conflict { .. }
        ));
    }

    #[test]
    fn the_plan_is_identical_under_reordered_input() {
        let tasks: Vec<Task> = (0..12)
            .map(|i| {
                let mut t = task(&format!("t{i:02}"), i);
                t.priority = (i as i64) % 3;
                t.resources.cpu_units = 4;
                t
            })
            .collect();
        let forward = mk_catalog(tasks.clone());
        let mut reversed_tasks = tasks;
        reversed_tasks.reverse();
        let reversed = mk_catalog(reversed_tasks);
        let capacity = Capacity {
            executor_slots: 3,
            ..wide()
        };
        let first = select(&input(
            &forward,
            &capacity,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        let second = select(&input(
            &reversed,
            &capacity,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[],
            &policy(),
        ));
        assert_eq!(first, second, "listing order must not reach the plan");
    }
}
