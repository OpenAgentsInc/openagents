//! Fresh account, resident host, and retained task evidence for browser acceptance.
//! The worker has a synthetic home. Optional control rights affect only scratch tasks.

#[allow(dead_code)]
#[path = "cloud_session_fixture.rs"]
mod account_fixture;
#[allow(dead_code)]
#[path = "support/operator_fixture.rs"]
mod operator_fixture;
#[allow(dead_code)]
#[path = "support/project_fixture.rs"]
mod project_fixture;
#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

use coder_host::Tasks;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|_| "synthetic private file could not be created".into())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("fixture clock")
        .as_secs()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let worker = args.first().is_some_and(|arg| arg == "--synthetic-worker");
    let args = if worker { &args[1..] } else { &args[..] };
    let (directory, build, listen, controls, services) = match args {
        [directory, build, listen] => (directory, build, listen, false, false),
        [directory, build, listen, mode] if mode == "controls" || mode == "services" => (directory, build, listen, true, mode == "services"),
        _ => return Err("usage: cloud_task_fixture NEW_SCRATCH_DIRECTORY CLOUD_WASM_DIRECTORY 127.0.0.1:PORT [controls|services]".into()),
    };
    let directory = PathBuf::from(directory);
    if !directory.is_absolute() || !Path::new(build).is_absolute() {
        return Err("synthetic fixture paths must be absolute".into());
    }
    if !worker {
        std::fs::create_dir(&directory).map_err(|_| "synthetic fixture directory must be new")?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "synthetic fixture directory permissions failed")?;
        let home = directory.join("synthetic-home");
        std::fs::create_dir(&home).map_err(|_| "synthetic home creation failed")?;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "synthetic home permissions failed")?;
        // Set HOME only on this new child. The real owner's environment and
        // stores are never changed or opened by this fixture.
        let status = std::process::Command::new(
            std::env::current_exe().map_err(|_| "fixture executable is unavailable")?,
        )
        .arg("--synthetic-worker")
        .args(args)
        .env("HOME", &home)
        .env_remove("OPENAGENTS_HOST_READY_FILE")
        .env_remove("OPENAGENTS_HOST_VERSION")
        .status()
        .map_err(|_| "synthetic worker could not start")?;
        return if status.success() {
            Ok(())
        } else {
            Err("synthetic worker failed".into())
        };
    }
    let directory = directory
        .canonicalize()
        .map_err(|_| "synthetic directory is unavailable")?;
    if std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|path| path.canonicalize().ok())
        != Some(directory.join("synthetic-home"))
    {
        return Err("synthetic worker requires its own fresh home".into());
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| "fixture runtime failed")?
        .block_on(serve(&directory, build, listen, controls, services))
}

async fn serve(
    directory: &Path,
    build: &str,
    listen: &str,
    controls: bool,
    services: bool,
) -> Result<(), String> {
    let root = directory.join("resident-checkout");
    std::fs::create_dir(&root).map_err(|_| "synthetic checkout creation failed")?;
    let store = directory.join("resident-tasks");
    let workspaces = BTreeMap::from([("checkout".into(), root.clone())]);
    let inbox = Arc::new(
        coder::task::remote::Inbox::new(&store, workspaces.clone())
            .with_settings(directory.join("unused-resident-settings")),
    );
    let device = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let task = "c".repeat(64);
    // The durable task is inert. The synthetic owner below records a fixed
    // transcript and a candidate, without launching a provider or command.
    let mut native = coder::task::Store::open(&store).map_err(|_| "synthetic task store failed")?;
    let command = coder::task::Command { schema: coder::task::COMMAND_SCHEMA.into(), command_id: "synthetic-submit".into(), task_id: task.clone(), expected_revision: None,
        action: coder::task::Action::Submit { intent: coder::task::TaskIntent { title: "Synthetic resident evidence".into(), prompt: "Synthetic original request: retain **every** original record, show separate child references, and preserve candidate bytes.".into(),
            workspace: coder::task::Workspace { path: root.to_string_lossy().into_owned(), source_revision: None }, configuration: coder::task::RequestedConfiguration { adapter: coder::task::adapter::NAME.into(), model: Some("synthetic".into()) }, images: Vec::new() } } };
    native
        .apply(&serde_json::to_vec(&command).map_err(|_| "synthetic task encoding failed")?)
        .map_err(|_| "synthetic task submission failed")?;
    drop(native);
    coder::task::owner::allow_scripted(&store, "isolated browser acceptance with a synthetic home")
        .map_err(|_| "synthetic owner admission failed")?;
    coder::task::owner::scripted(&store, &task, |_, workspace| {
        std::fs::write(workspace.join("synthetic-result.txt"), b"Synthetic candidate artifact\nOriginal bytes remain exact.\n").map_err(|_| "synthetic candidate failed")?;
        let mut log = std::fs::OpenOptions::new().append(true).open(store.join(format!("{task}.1.atif.jsonl"))).map_err(|_| "synthetic trace failed")?;
        for step in [
            json!({"at":now()*1000,"source":"Agent","message":"Synthetic native tool call","call":{"id":"synthetic-tool-1","name":"shell","arguments":{"command":"synthetic recorded check"},"output":"Synthetic check output. No command was executed.","outcome":"Completed","milliseconds":0,"extra":{}},"extensions":{}}),
            json!({"at":now()*1000,"source":"Agent","message":"Synthetic structured child reference","call":{"id":"synthetic-child-call","name":"delegate","arguments":{"agent":"codex"},"output":"Synthetic child session reference; no child engine ran.","outcome":"Completed","milliseconds":0,"extra":{"schema":"openagents.delegate-call.v1","capability":"codex","session_id":"synthetic-child-session"}},"extensions":{}}),
        ] {
            serde_json::to_writer(&mut log, &json!({"record":"step","step":step})).map_err(|_| "synthetic trace encoding failed")?;
            log.write_all(b"\n").map_err(|_| "synthetic trace append failed")?;
        }
        log.sync_all().map_err(|_| "synthetic trace sync failed")?;
        Ok(coder::task::owner::Scripted { ending: "model_finished".into(), reply: "Synthetic oversized original reply. ".repeat(1500) })
    }).map_err(|_| "synthetic retained run failed")?;
    // Verify the fixture through the actual resident owner before exposing it.
    let page = inbox
        .task_read(&coder_access::task_read::PageQuery {
            workspace: "checkout".into(),
            task: task.clone(),
            revision: None,
            cursor: None,
            limit: 64,
        })
        .map_err(|_| "synthetic resident read failed")?;
    if page.evidence.state != "sealed" || page.children.len() != 1 {
        return Err("synthetic native evidence is incomplete".into());
    }

    let (relay_url, relay, _) = relay::start().await;
    let state = directory.join("resident-access");
    let authority = coder_access::host::Host::new(&state, coder_access::RelayPolicy::LoopbackTest);
    let owner = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    authority
        .init(&coder_access::protocol::pubkey(&owner))
        .map_err(|_| "synthetic host initialization failed")?;
    let invitation = authority
        .invite(
            &relay_url,
            coder_access::Rights::new(if controls {
                vec![coder_access::Right::Observe, coder_access::Right::Operate]
            } else {
                vec![coder_access::Right::Observe]
            })
            .map_err(|_| "synthetic fixture rights failed")?,
            now(),
            now() + 3600,
        )
        .map_err(|_| "synthetic invitation failed")?;
    let parsed = coder_access::protocol::HostInvitation::parse(
        &invitation.code,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "synthetic invitation parse failed")?;
    let pending = coder_access::client::prepare_redeem(
        &parsed,
        &device,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "synthetic redemption failed")?;
    let answer = authority
        .handle_redemption(&pending.event, || Ok(now()))
        .map_err(|_| "synthetic host redemption failed")?;
    let access = coder_access::client::finish_redeem(
        &parsed,
        &pending,
        &answer,
        &device,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "synthetic device access failed")?;
    if controls {
        let inert = "d".repeat(64);
        let mut native =
            coder::task::Store::open(&store).map_err(|_| "synthetic control store failed")?;
        let command = coder::task::Command {
            schema: coder::task::COMMAND_SCHEMA.into(),
            command_id: "synthetic-control-submit".into(),
            task_id: inert.clone(),
            expected_revision: None,
            action: coder::task::Action::Submit {
                intent: coder::task::TaskIntent {
                    title: "Synthetic inert controls".into(),
                    prompt: "Synthetic pending task. No engine has execution authority.".into(),
                    workspace: coder::task::Workspace {
                        path: root.to_string_lossy().into_owned(),
                        source_revision: None,
                    },
                    configuration: coder::task::RequestedConfiguration {
                        adapter: coder::task::adapter::NAME.into(),
                        model: None,
                    },
                    images: vec![],
                },
            },
        };
        native
            .apply(&serde_json::to_vec(&command).map_err(|_| "synthetic control encoding failed")?)
            .map_err(|_| "synthetic inert control submission failed")?;
        drop(native);
        let principal = coder_host::Principal {
            device: coder_access::protocol::pubkey(&device),
            grant: Some(access.grant.grant.clone()),
            epoch: Some(access.grant.epoch),
        };
        inbox
            .command_at_revision(
                &principal,
                &coder_access::protocol::TaskCommand {
                    command: "e".repeat(64),
                    task: inert.clone(),
                    action: coder_access::protocol::CommandAction::Queue,
                    based_on: 1,
                    text: "Synthetic held message for queue controls.".into(),
                    emulate: false,
                    issued_at: now(),
                },
                1,
                &|_| true,
            )
            .map_err(|_| "synthetic queue submission failed")?;
        println!(
            "{}",
            json!({"synthetic":true,"control_task":inert,"control_route":format!("/cloud/app/hosts/resident/tasks/{inert}/actions")})
        );
    }
    let mut host = coder_host::config::Config::new(state, vec![relay_url], 7);
    host.policy = coder_access::RelayPolicy::LoopbackTest;
    let inbox = if services {
        let device_id = coder_access::protocol::pubkey(&device);
        let projects = project_fixture::observer(directory, &device_id, &workspaces)?;
        let check = coder_host::cloud::authority(authority)
            .map_err(|_| "synthetic cloud standing failed")?;
        let cloud = operator_fixture::operator(directory, check, &device_id, &workspaces)?;
        println!(
            "{}",
            json!({"synthetic":true,"project_route":"/cloud/app/hosts/resident/projects","operator_route":"/cloud/app/hosts/resident/cloud","operator_project":operator_fixture::PROJECT,"operator_profile":operator_fixture::PROFILE})
        );
        Arc::new(
            (*inbox)
                .clone()
                .with_projects(Arc::new(projects))
                .with_cloud(Arc::new(cloud)),
        )
    } else {
        inbox
    };
    host.workspaces = workspaces;
    host.telemetry = false;
    let running = coder_host::start(host, inbox)
        .await
        .map_err(|_| "synthetic resident host failed")?;
    let secret = directory.join("resident-device.key");
    let access_path = directory.join("resident-device.access");
    private_file(&secret, &device.secret_bytes())?;
    private_file(
        &access_path,
        &serde_json::to_vec(&access).map_err(|_| "synthetic access encoding failed")?,
    )?;
    let hosts = directory.join("hosts.json");
    let mut document = json!({"schema":"openagents.cloud.host-bindings.v1","bindings":[{"id":"resident","account":"alice","workspace":"alice-personal","members_epoch":3,"host_workspace":"checkout","host_generation":7,"route":format!("tcp://{}",running.local_addr()),"access_file":access_path,"device_secret":secret}]});
    if controls {
        let journal = directory.join("browser-controls");
        std::fs::create_dir(&journal).map_err(|_| "synthetic control journal creation failed")?;
        std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "synthetic control journal permissions failed")?;
        document["controls"] = json!({"directory":journal,"bindings":["resident"]});
    }
    private_file(
        &hosts,
        &serde_json::to_vec(&document).map_err(|_| "synthetic binding encoding failed")?,
    )?;
    println!(
        "{}",
        json!({"synthetic":true,"task":task,"task_route":format!("/cloud/app/hosts/resident/tasks/{task}"),"device_right":if controls { "observe,operate" } else { "observe" }})
    );
    let result = account_fixture::serve(
        directory
            .join("account-web")
            .to_str()
            .ok_or("synthetic account path failed")?,
        build,
        listen,
        hosts.to_str(),
    )
    .await;
    running.shutdown().await;
    relay.abort();
    result
}
