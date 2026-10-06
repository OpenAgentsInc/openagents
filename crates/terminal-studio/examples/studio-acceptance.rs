//! A bounded scripted studio capture through the existing host operations.
use coder::task::{Store, studio_sim as sim};
use coder_access::{
    Operation, Outcome, Right,
    studio::{DecisionKind, Snapshot, TaskStatus},
};
use openagents_connect::control::OperationClient;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use terminal_core::studio::{Page, Section};
use terminal_studio::{
    opening::{self, Target},
    studio,
};

const RIGHTS: &[Right] = &[Right::Observe, Right::Operate, Right::Review];

fn read(client: &mut OperationClient) -> Result<Snapshot, String> {
    match client
        .call(
            &coder_access::studio_intents::mint(),
            &Operation::StudioSnapshot {},
        )
        .map_err(|e| e.to_string())?
    {
        Outcome::Studio { snapshot } => Ok(*snapshot),
        _ => Err("Expected the scratch studio snapshot.".into()),
    }
}

fn capture(page: &Page) -> Vec<String> {
    terminal_core::studio::lines(page)
        .into_iter()
        .map(|(line, _)| line)
        .collect()
}

fn enter(
    page: &mut Page,
    client: &mut OperationClient,
    line: &str,
    events: &mut Vec<Value>,
) -> Result<Outcome, String> {
    page.enter(line.to_owned());
    let line = page
        .prepare
        .take()
        .ok_or("Sheet did not prepare this line.")?;
    let snapshot: Snapshot =
        serde_json::from_slice(&page.prepare_source).map_err(|e| e.to_string())?;
    let prepared = studio::prepare(
        page.prepare_review.as_ref(),
        &snapshot,
        RIGHTS,
        &line,
        page.workspace.as_deref(),
    )?;
    page.prepared(Ok(prepared));
    let confirmation = capture(page);
    page.enter(String::new());
    let prepared = page.send.take().ok_or("Sheet did not confirm this line.")?;
    page.enter(String::new());
    if page.send.is_some() {
        return Err("A repeated Enter dispatched twice.".into());
    }
    let operation: Operation =
        serde_json::from_slice(&prepared.bytes).map_err(|e| e.to_string())?;
    let outcome = client
        .call(&prepared.request, &operation)
        .map_err(|e| e.to_string())?;
    // Exact transport redelivery must return the same host answer.
    let repeated = client
        .call(&prepared.request, &operation)
        .map_err(|e| e.to_string())?;
    if repeated != outcome {
        return Err("Repeated delivery changed its host answer.".into());
    }
    events.push(
        json!({"request":prepared.request,"operation":operation,"outcome":outcome,
        "repeated_delivery_equal":true,"confirmation":confirmation}),
    );
    Ok(outcome)
}

fn binary_digest() -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(std::env::current_exe()?)?;
    let mut digest = Sha256::new();
    let mut bytes = [0; 64 * 1024];
    loop {
        let read = file.read(&mut bytes)?;
        if read == 0 {
            break;
        }
        digest.update(&bytes[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn git_head(root: &Path) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Scratch Git HEAD was unavailable.".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn flow(
    scratch: &sim::Scratch,
    client: &mut OperationClient,
    output: &Path,
) -> Result<Value, String> {
    let engine = sim::Engine::open(&scratch.root, &scratch.store).map_err(|e| e.to_string())?;
    let before = git_head(&scratch.fixture.origin)?;
    let mut events = Vec::new();
    let mut page = Page::default();
    page.open = true;
    page.external = true;
    page.update(studio::project(&read(client)?, RIGHTS)?)?;
    page.enter(format!("/repo {}", sim::WORKSPACE));
    enter(&mut page, client, sim::GOAL, &mut events)?;
    let mut opened = false;
    let mut answered = false;
    let mut turns = Vec::new();
    for pass in 0..32 {
        turns.extend(engine.step().map_err(|e| e.to_string())?);
        let snapshot = read(client)?;
        if snapshot.view.goals.len() != 1 {
            return Err("Goal dispatch was duplicated.".into());
        }
        page.update(studio::project(&snapshot, RIGHTS)?)?;
        if !opened {
            let seat = snapshot
                .view
                .seats
                .iter()
                .find(|seat| seat.task.is_some())
                .ok_or("No active seat in the scratch studio.")?;
            let keyboard = opening::context(&snapshot, RIGHTS, &Target::Desk(seat.desk), None)?;
            let workshop =
                opening::context(&snapshot, RIGHTS, &Target::Seat(seat.seat.clone()), None)?;
            if keyboard != workshop {
                return Err("Entry points selected different resources.".into());
            }
            events.push(
                json!({"keyboard_context":keyboard,"workshop_context":workshop,
                "capture_kind":"headless_shared_sheet_semantics","sheet":capture(&page)}),
            );
            page.open = false;
            page.open = true;
            if page.send.is_some() || page.prepare.is_some() {
                return Err("Reopening dispatched work.".into());
            }
            *client = OperationClient::new(scratch.socket.clone());
            let reconnected = read(client)?;
            if reconnected.stream != snapshot.stream {
                return Err("Source identity changed on reconnect.".into());
            }
            page.update(studio::project(&reconnected, RIGHTS)?)?;
            events.push(
                json!({"reopened":true,"reconnected_stream":reconnected.stream,
                "new_dispatch":false}),
            );
            opened = true;
        }
        if let Some(decision) = snapshot
            .view
            .decisions
            .iter()
            .find(|d| d.kind == DecisionKind::Question)
        {
            page.section = Section::Decisions;
            enter(
                &mut page,
                client,
                &format!("/answer {} {}", decision.decision, sim::ANSWER),
                &mut events,
            )?;
            answered = true;
            continue;
        }
        if let Some(task) = snapshot
            .view
            .tasks
            .iter()
            .find(|task| task.entry == "greet" && task.status == TaskStatus::Done)
        {
            let Outcome::Review { review } = client
                .call(
                    &coder_access::studio_intents::mint(),
                    &Operation::OpenReview {
                        task: task.task.clone(),
                    },
                )
                .map_err(|e| e.to_string())?
            else {
                return Err("Host answered another review.".into());
            };
            page.section = Section::Review;
            page.reviewed(Ok(studio::project_review(&snapshot.stream, &review)?));
            let task_record = Store::open(&scratch.store)
                .map_err(|e| e.to_string())?
                .show(&task.task)
                .map_err(|e| e.to_string())?;
            let artifact_dir = output.join("artifacts");
            std::fs::create_dir_all(&artifact_dir).map_err(|e| e.to_string())?;
            let manifest = coder::task::artifact::manifest(&scratch.store, &task_record)
                .map_err(|e| e.to_string())?;
            if let Some(manifest) = &manifest {
                std::fs::write(
                    artifact_dir.join("manifest.json"),
                    serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                for entry in &manifest.entries {
                    if entry.digest.is_none() {
                        continue;
                    }
                    if entry
                        .path
                        .components()
                        .any(|part| !matches!(part, std::path::Component::Normal(_)))
                    {
                        return Err("Fixture artifact has an unsafe retained path.".into());
                    }
                    let retained =
                        coder::task::artifact::read(&scratch.store, &task.task, &entry.path)
                            .map_err(|e| e.to_string())?;
                    let path = artifact_dir.join(&entry.path);
                    std::fs::create_dir_all(path.parent().ok_or("Artifact has no parent.")?)
                        .map_err(|e| e.to_string())?;
                    std::fs::write(path, retained).map_err(|e| e.to_string())?;
                }
            }
            let trace_file = task_record.trace_file(task_record.turn());
            std::fs::copy(
                scratch.store.join(&trace_file),
                artifact_dir.join(&trace_file),
            )
            .map_err(|e| e.to_string())?;
            events.push(json!({"retained_artifact_manifest":manifest,"retained_trace":trace_file}));
            events.push(json!({"review":review,"sheet":capture(&page),
                "task_checks":task_record.checks,"run_evidence":task_record.run}));
            let outcome = enter(&mut page, client, "/merge", &mut events)?;
            let Outcome::Merged { merged } = &outcome else {
                return Err("Host did not answer the merge.".into());
            };
            let publication = merged
                .publication
                .as_ref()
                .ok_or("Merge has no landing record.")?;
            if publication.state != coder_access::review::PublishState::Published {
                return Err(format!("Local landing failed: {}", publication.note));
            }
            let after = git_head(&scratch.fixture.origin)?;
            if before != after {
                return Err("Local merge changed the scratch remote.".into());
            }
            if !answered {
                return Err("No question was answered before merging.".into());
            }
            return Ok(
                json!({"status":"passed","passes":pass+1,"events":events,"scripted_turns":turns,
                "origin_before":before,"origin_after":after,"pushes_during_merge":0,
                "local_head":git_head(&scratch.fixture.checkout)?,"task":task.task,
                "observed_host_spend":snapshot.view.goals[0].spend}),
            );
        }
    }
    Err("Scripted studio exceeded 32 passes without a reviewed local merge.".into())
}

fn archive(scratch: &sim::Scratch, client: &mut OperationClient) -> Result<Vec<String>, String> {
    let tasks = Store::open(&scratch.store)
        .map_err(|e| e.to_string())?
        .list()
        .map_err(|e| e.to_string())?;
    let mut archived = Vec::new();
    for task in tasks {
        if !matches!(
            task.status,
            coder::task::Status::Finished | coder::task::Status::Cancelled
        ) {
            client
                .call(
                    &coder_access::studio_intents::mint(),
                    &Operation::CancelTask {
                        task: task.task_id.clone(),
                        revision: task.revision,
                        reason: "End the isolated workbench acceptance capture.".into(),
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        client
            .call(
                &coder_access::studio_intents::mint(),
                &Operation::ArchiveTask {
                    task: task.task_id.clone(),
                },
            )
            .map_err(|e| e.to_string())?;
        archived.push(task.task_id);
    }
    Ok(archived)
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Usage: studio-acceptance SOURCE_COMMIT OUTPUT_DIRECTORY".into());
    }
    coder_access::review::revision(&args[1])?;
    let output = PathBuf::from(&args[2]);
    std::fs::create_dir_all(&output)?;
    let temp = tempfile::tempdir()?;
    let scratch = sim::Scratch::create(&temp.path().join("studio"))?;
    scratch.seat_team()?;
    let workspaces =
        BTreeMap::from([(sim::WORKSPACE.to_owned(), scratch.fixture.checkout.clone())]);
    let policy = coder_access::RelayPolicy::LoopbackTest;
    let host = coder_access::host::Host::new(&scratch.state, policy);
    let host_id = host.init(&"a".repeat(64))?;
    let mut config =
        coder_host::config::Config::new(scratch.state.clone(), vec!["ws://127.0.0.1:9".into()], 1);
    config.policy = policy;
    config.telemetry = false;
    config.advertise_listener = false;
    config.workspaces = workspaces.clone();
    config.control = Some(coder_host::config::Control {
        path: scratch.socket.clone(),
        root: scratch.root.clone(),
        autostart: None,
        tasks: scratch.store.clone(),
        uid: coder_host::control::own_uid(),
    });
    let running = coder_host::start(
        config,
        Arc::new(sim::inbox(&scratch.store, &scratch.root, &workspaces)),
    )
    .await?;
    let started = Instant::now();
    let capture_binary = binary_digest()?;
    let mut client = OperationClient::new(scratch.socket.clone());
    let result = flow(&scratch, &mut client, &output);
    let cleanup = archive(&scratch, &mut client);
    running.shutdown().await;
    let passed = result.is_ok() && cleanup.is_ok();
    let receipt = json!({"schema":"openagents.workbench.studio-acceptance.v1","simulated":true,
        "source_commit":args[1],"app":{"component":"terminal-core shared sheet","capture_version":env!("CARGO_PKG_VERSION"),
            "capture_binary_sha256":capture_binary,"renderer":"none; semantic capture"},
        "host":{"pubkey":host_id,"implementation":"coder-host in-process scratch", "protocol":openagents_connect::control::VERSION},
        "engine":"existing scripted studio engine; no model call","elapsed_ms":started.elapsed().as_millis(),
        "limit":{"scripted_passes":32,"socket_timeout_seconds":10},
        "flow":result.unwrap_or_else(|error| json!({"status":"failed","error":error})),
        "archived_tasks":cleanup.as_ref().ok(),"cleanup_error":cleanup.err(),
        "native_key_and_render_acceptance":"unverified; semantic capture only",
        "real_engine_receipt":{"status":"unverified","reason":"separate bounded owner qualification required"}});
    let bytes = serde_json::to_vec_pretty(&receipt)?;
    std::fs::write(output.join("simulated.json"), &bytes)?;
    println!(
        "{}",
        if passed {
            "Scripted studio acceptance passed; tasks archived; no model call."
        } else {
            "Scripted studio acceptance failed; inspect simulated.json."
        }
    );
    if !passed {
        return Err("Scripted studio acceptance did not pass.".into());
    }
    Ok(())
}
