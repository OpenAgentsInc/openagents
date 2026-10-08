//! Retained supervisor evidence for isolated route and browser acceptance.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use coder_project::controller::{Configuration, Prepared};
use coder_scheduler::{catalog, plan, resources};
use serde_json::{Value, json};

fn write(path: &Path, value: &Value) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_vec(value).map_err(|_| "synthetic project encoding failed")?,
    )
    .map_err(|_| "synthetic project write failed")?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| "synthetic project protection failed".into())
}
fn mkdir(path: &Path) -> Result<(), String> {
    std::fs::create_dir(path).map_err(|_| "synthetic project directory failed")?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "synthetic project protection failed".into())
}
pub fn observer(
    directory: &Path,
    device: &str,
    workspaces: &BTreeMap<String, PathBuf>,
) -> Result<coder_project::observe::Observer, String> {
    let repository = workspaces
        .get("checkout")
        .ok_or("synthetic project workspace unavailable")?;
    let state = directory.join("project-supervisor");
    mkdir(&state)?;
    mkdir(&state.join("snapshots"))?;
    mkdir(&state.join("attempts"))?;
    mkdir(&state.join("reviewed"))?;
    let base = "a".repeat(40);
    let mut tasks = Vec::new();
    for (id, issue, dependencies) in [
        ("held", 101, vec![]),
        ("dependent", 102, vec!["held".into()]),
        ("review", 103, vec![]),
    ] {
        let mut prepared = Prepared {
            scheduling: catalog::Task {
                id: id.into(),
                issue,
                base: base.clone(),
                input: String::new(),
                depends_on: dependencies,
                footprint: catalog::Footprint::Declared {
                    reads: vec!["src/lib.rs".into()],
                    writes: vec![],
                },
                priority: 0,
                resources: resources::Resources::default(),
                estimate_ticks: 1,
            },
            assignment: coder_project::Assignment {
                id: id.into(),
                base: base.clone(),
                prompt: "Synthetic original prepared work. ".repeat(600),
                writes: false,
                expected_text: None,
                minutes: 1,
            },
            issue_updated: "2026-10-08T00:00:00Z".into(),
            issue_body_digest: atif::digest(&json!(issue)),
            tracker_base: base.clone(),
        };
        prepared.scheduling.input = prepared.input_digest();
        tasks.push(prepared);
    }
    let configuration = Configuration {
        v: 1,
        repository: repository.clone(),
        project: coder_project::github::Scope {
            owner: "OpenAgentsInc".into(),
            repository: "synthetic".into(),
            project: 19,
        },
        capacity: resources::Capacity {
            executor_slots: 4,
            cpu_units: 8,
            memory_mib: 2048,
            integration_lanes: 1,
        },
        external_reservation: resources::Resources {
            executor_slots: 1,
            cpu_units: 1,
            memory_mib: 128,
            quiet_host: false,
            integration: false,
        },
        excluded_issues: BTreeSet::from([104]),
        external_owners: vec![plan::Exclusion {
            owner: "another-owner".into(),
            writes: vec!["src".into()],
        }],
        accepted_closed_issues: BTreeSet::from([100]),
        review_cap: 1,
        dispatch_limit: 2,
        admission_minutes: 10,
        poll_seconds: 10,
        quota_backoff_seconds: 300,
        tasks,
    };
    configuration.validate()?;
    let config = directory.join("project-supervisor.json");
    write(&config, &json!(configuration))?;
    write(
        &state.join("scheduler-ledger.json"),
        &json!({"v":coder_scheduler::ledger::SCHEMA,"run":"synthetic-supervisor","sequence":9,"next_attempt":3,"tasks":{
            "held":{"status":"active","owner":"another-owner","attempt":"synthetic-1","task_digest":configuration.tasks[0].scheduling.digest(),"attempts":1,"updated_unix":10},
            "review":{"status":"review","owner":"review-owner","attempt":"synthetic-2","task_digest":configuration.tasks[2].scheduling.digest(),"attempts":1,"updated_unix":11}
        }}),
    )?;
    write(
        &state.join("snapshots/observer-000001.json"),
        &json!({"round":1,"blocked":{"held":"Claim held by another owner; observation does not take it","dependent":"Dependency held has not completed","review":"Independent review remains pending"},"unprepared_issues":[105],"snapshot":{"source_digest":atif::digest(&json!("synthetic retained tracker")),"issues":{"101":{"number":101,"closed":false},"102":{"number":102,"closed":false,"blockers":[{"number":101,"closed":false}]}}}}),
    )?;
    let policy = directory.join("project-observer.json");
    write(
        &policy,
        &json!({"schema":"openagents.project.observe.v1","projects":[{"id":"synthetic-project","workspace":"checkout","label":"Synthetic supervised project","configuration":config,"state":state,"devices":[device]}]}),
    )?;
    coder_project::observe::Observer::load(&policy, workspaces)
}
