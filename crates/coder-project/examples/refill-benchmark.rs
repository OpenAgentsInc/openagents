//! Compare fixed waves with completion-driven refill using real bounded fixture processes.
//! This benchmark does not call an inference provider.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder::{Bounds, Delegator, Executor, Policy, Task};
use serde_json::{Value, json};

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn event(path: &Path, phase: &str, id: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    file.lock().unwrap();
    writeln!(file, "{}", json!({"phase":phase,"id":id,"at_ms":millis()})).unwrap();
    file.unlock().unwrap();
}

fn fixture(args: &[String]) {
    let id = args.last().unwrap();
    let events = Path::new(&args[2]);
    event(events, "start", id);
    std::thread::sleep(Duration::from_millis(if id == "task-00" {
        400
    } else {
        35
    }));
    event(events, "end", id);
    println!("{id}");
}

#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--fixture") {
        fixture(&args);
        return;
    }
    if !coder::delegate::boundary_supported() {
        eprintln!("refill benchmark requires an enforcing filesystem boundary");
        std::process::exit(2);
    }
    let binary = std::env::current_exe().unwrap();
    let mut rows = Vec::new();
    for width in [1, 2, 4, 6, 8, 10] {
        for trial in 0..3 {
            let modes = if trial % 2 == 0 {
                ["fixed-waves", "refill"]
            } else {
                ["refill", "fixed-waves"]
            };
            for mode in modes {
                let state = tempfile::tempdir().unwrap();
                let events = state.path().join("events.jsonl");
                let executor = Executor::new(
                    "refill-fixture",
                    &binary,
                    vec!["--fixture".into(), events.display().to_string()],
                )
                .under(Policy::empty().granting(state.path()));
                let workspace = tempfile::tempdir().unwrap();
                let delegator = Delegator::new(executor)
                    .in_directory(workspace.path())
                    .bounded_to(width);
                let tasks: Vec<_> = (0..12)
                    .map(|index| {
                        let id = format!("task-{index:02}");
                        Task::reading(&id, "fixture.rs")
                            .expecting(&id)
                            .bounded(Bounds::within(Duration::from_secs(10)))
                    })
                    .collect();
                let origin = millis();
                let start = Instant::now();
                let mut results = Vec::new();
                if mode == "fixed-waves" {
                    for wave in tasks.chunks(width) {
                        results.extend(delegator.fan_out(wave.to_vec()).await);
                    }
                } else {
                    results = delegator.fan_out(tasks).await;
                }
                let elapsed = start.elapsed().as_millis();
                let passed = results
                    .iter()
                    .filter(|r| r.answered() && r.correct() == Some(true))
                    .count();
                if passed != 12 {
                    eprintln!("fixture failed at width {width}: {results:?}");
                    std::process::exit(1);
                }
                let records: Vec<Value> = std::fs::read_to_string(&events)
                    .unwrap()
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect();
                let mut active = 0;
                let mut peak = 0;
                let mut waits = Vec::new();
                for record in records {
                    if record["phase"] == "start" {
                        active += 1;
                        peak = peak.max(active);
                        waits.push(
                            u128::from(record["at_ms"].as_u64().unwrap()).saturating_sub(origin),
                        );
                    } else {
                        active -= 1;
                    }
                    assert!(active >= 0);
                }
                assert_eq!(active, 0);
                assert!(peak <= width as i32);
                rows.push(json!({"width":width,"mode":mode,"trial":trial+1,"accepted_fixture_results":passed,"makespan_ms":elapsed,"peak_fixture_processes":peak,"queue_to_child_start_ms":waits}));
            }
        }
    }
    println!("{}", serde_json::to_string_pretty(&json!({
        "schema":"openagents.refill-process-benchmark.v1", "kind":"local-fixture-processes",
        "external_inference":false,"os":std::env::consts::OS,
        "available_parallelism":std::thread::available_parallelism().unwrap().get(),
        "source_digest":atif::digest(&json!(include_str!("refill-benchmark.rs"))),
        "workload":{"tasks":12,"first_sleep_ms":400,"other_sleep_ms":35},
        "limits":"Observed fixture process timing on a shared host; not Devin throughput or quiet-host inference evidence.",
        "rows":rows
    })).unwrap());
}
