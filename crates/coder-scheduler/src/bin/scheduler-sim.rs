//! `scheduler-sim`: a reproducible scheduling benchmark.
//!
//! Builds one fixed mixed catalog — software edits, builds, and
//! quiet-host work — and runs it under both refill policies at executor
//! capacities 1, 2, 4, 6, 8, and 10. The output is one JSON document on
//! stdout: makespan in simulated ticks, per-policy queue waits, and the
//! busiest the host's declared resources became.
//!
//! Everything is deterministic — the catalog is a function of its index
//! arithmetic, not of a random seed — so two invocations print identical
//! bytes. The ticks are stated estimates, not wall clock: the report
//! compares the two policies against each other and makes no claim about
//! live speed.

use std::collections::BTreeSet;

use coder_scheduler::catalog::{Catalog, Footprint, Task};
use coder_scheduler::resources::{Capacity, Resources};
use coder_scheduler::simulate::{self, Fill};
use serde::Serialize;

/// The executor-slot capacities the benchmark sweeps.
const CAPACITIES: &[u32] = &[1, 2, 4, 6, 8, 10];

/// One run's row in the report.
#[derive(Serialize)]
struct Row {
    capacity_executor_slots: u32,
    policy: &'static str,
    makespan_ticks: u64,
    rounds: u32,
    scheduled: u32,
    unscheduled: usize,
    mean_queue_wait_ticks: f64,
    max_queue_wait_ticks: u64,
    peak_executor_slots: u32,
    peak_cpu_units: u32,
    peak_memory_mib: u64,
    quiet_host_held: bool,
}

/// The report printed on stdout.
#[derive(Serialize)]
struct Report {
    v: &'static str,
    catalog: &'static str,
    catalog_digest: String,
    tasks: usize,
    note: &'static str,
    runs: Vec<Row>,
}

/// The fixed benchmark catalog: thirty tasks mixing software edits,
/// builds, and quiet-host work, with a dependency chain, one deliberate
/// write collision, and reads shared across the tree.
fn bench_catalog() -> Catalog {
    let mut tasks = Vec::new();
    for i in 0..30_u64 {
        let id = format!("bench-{i:02}");
        let mut task = Task {
            id: id.clone(),
            issue: 9000 + i,
            base: "sha256:bench-base".to_string(),
            input: format!("sha256:bench-input-{i:02}"),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec!["Cargo.toml".to_string()],
                writes: vec![format!("crates/bench/{id}/src/lib.rs")],
            },
            priority: 0,
            resources: Resources {
                executor_slots: 1,
                cpu_units: 2,
                memory_mib: 512,
                quiet_host: false,
                integration: false,
            },
            estimate_ticks: 3 + i % 5,
        };
        match i % 10 {
            // Quiet-host measurement work: runs alone on the host.
            0 => {
                task.resources.quiet_host = true;
                task.resources.cpu_units = 4;
                task.estimate_ticks = 6;
            }
            // Builds: the heavyweight lane.
            4 | 7 => {
                task.resources.cpu_units = 8;
                task.resources.memory_mib = 4096;
                task.estimate_ticks = 8 + i % 4;
                task.footprint = Footprint::Declared {
                    reads: vec!["Cargo.toml".to_string()],
                    writes: vec![format!("target/bench/{id}")],
                };
            }
            _ => {}
        }
        // Integrating tasks serialize through the one integration lane.
        if i % 6 == 5 {
            task.resources.integration = true;
        }
        tasks.push(task);
    }
    // A dependency chain, so ordering is exercised.
    tasks[7].depends_on = vec!["bench-03".to_string()];
    tasks[14].depends_on = vec!["bench-07".to_string()];
    tasks[21].depends_on = vec!["bench-14".to_string()];
    // One deliberate write-write collision: two tasks edit the shared
    // roll-up document and must serialize.
    tasks[9].footprint = Footprint::Declared {
        reads: vec!["Cargo.toml".to_string()],
        writes: vec!["docs/bench-roll-up.md".to_string()],
    };
    tasks[18].footprint = Footprint::Declared {
        reads: vec![],
        writes: vec!["docs/bench-roll-up.md".to_string()],
    };
    // A priority bump on one mid-list task, so ordering is not just
    // issue order.
    tasks[25].priority = 10;
    let mut catalog = Catalog::new("backlog-bench", tasks);
    catalog.seal().expect("the benchmark catalog validates");
    catalog
}

fn main() {
    let catalog = bench_catalog();
    let mut runs = Vec::new();
    for &slots in CAPACITIES {
        let capacity = Capacity {
            executor_slots: slots,
            cpu_units: 32,
            memory_mib: 32768,
            integration_lanes: 1,
        };
        for fill in [Fill::Waves, Fill::Refill] {
            let report = simulate::run(&catalog, &capacity, fill, &BTreeSet::new());
            runs.push(Row {
                capacity_executor_slots: slots,
                policy: fill.name(),
                makespan_ticks: report.makespan_ticks,
                rounds: report.rounds,
                scheduled: report.scheduled,
                unscheduled: report.unscheduled.len(),
                mean_queue_wait_ticks: report.mean_queue_wait_ticks,
                max_queue_wait_ticks: report.max_queue_wait_ticks,
                peak_executor_slots: report.peak.executor_slots,
                peak_cpu_units: report.peak.cpu_units,
                peak_memory_mib: report.peak.memory_mib,
                quiet_host_held: report.peak.quiet_host_held,
            });
        }
    }
    let report = Report {
        v: "openagents.scheduler.sim.v1",
        catalog: "backlog-bench",
        catalog_digest: catalog.digest.clone(),
        tasks: catalog.tasks.len(),
        note: "simulated ticks over pinned estimates — a comparison of the \
             two refill policies, not a live-speedup claim",
        runs,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("the report serializes")
    );
}
