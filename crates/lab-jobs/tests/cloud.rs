#![cfg(all(feature = "cloud", unix))]
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::Request,
    response::{IntoResponse, Response},
};
use lab_jobs::{
    Capabilities, EffectiveRequirements, Placement,
    authority::{Enrolled, Worker},
    cloud::{Launcher, Progress, Settings},
    worker::Handler,
};
use openagents_sdk::{CancellationToken, Client, Deployment, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const ID: &str = "cmp_0123456789abcdef0123";
const COMMAND: &str = "trn_cloud_fixture";
const NOW: u64 = 1_800_000_000_000;
fn account_token() -> String {
    format!("sess_{}", "a".repeat(64))
}
fn requirements() -> EffectiveRequirements {
    EffectiveRequirements {
        cpu_millis: 1000,
        memory_bytes: 64 * 1024 * 1024,
        gpu: false,
        tools: BTreeSet::new(),
        max_seconds: 60,
        private_local: false,
        local_assets: BTreeSet::new(),
        no_training: true,
        job: "cad/echo".into(),
    }
}
fn enrolled() -> Enrolled {
    Enrolled {
        worker: Worker {
            id: "worker_fixture".into(),
            account: "acct_fixture".into(),
            space: lab_kit::SpaceRef::Personal {
                account: "acct_fixture".into(),
            },
            capabilities: Capabilities {
                worker: "worker_fixture".into(),
                placement: Placement::Cloud,
                cpu_millis: 1000,
                memory_bytes: 64 * 1024 * 1024,
                gpu: false,
                tools: BTreeSet::new(),
                jobs: BTreeSet::from(["cad/echo".into()]),
                local_assets: BTreeSet::new(),
                max_seconds: 60,
            },
            expires_unix_ms: NOW + 600_000,
            revoked: false,
            issuer_session: Some("session_reference".into()),
        },
        token: "labw_worker_fixture.job_only_fixture".into(),
    }
}
fn settings(guest: &Path) -> Settings {
    let executable = guest.join("labs-worker");
    fs::write(&executable, "#!/bin/sh\nset -eu\n[ \"$1\" = --config ]\n[ \"$3\" = --once ]\nprintf '%s' \"$2\" > \"$(dirname \"$0\")/config-path\"\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    Settings {
        environment: "cad-worker".into(),
        version: "worker-image-v1".into(),
        size: "small".into(),
        executable_sha256: format!("{:x}", Sha256::digest(fs::read(&executable).unwrap())),
        executable,
        workspace: guest.into(),
        handlers: BTreeMap::from([(
            "cad/echo".into(),
            Handler {
                executable: "/usr/bin/true".into(),
                arguments: vec![],
            },
        )]),
    }
}
#[derive(Default)]
struct Seen {
    creates: Vec<(String, Value)>,
    commands: Vec<(String, Value)>,
    effects: usize,
    deletes: usize,
    deleted_reads: usize,
    lose_create: bool,
    lose_command: bool,
    lose_delete: bool,
    run_bootstrap: bool,
}
#[derive(Clone)]
struct Fixture(Arc<Mutex<Seen>>);
async fn serve(State(fixture): State<Fixture>, request: Request<Body>) -> Response {
    assert_eq!(
        request.headers()["authorization"],
        format!("Bearer {}", account_token())
    );
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.unwrap();
    let path = parts.uri.path();
    let mut state = fixture.0.lock().unwrap();
    let response = if path == "/v1/account" {
        json!({"account":{"id":"acct_fixture","label":"Test"},"workspaces":[]})
    } else if parts.method == "POST" && path == "/v1/computers" {
        state.creates.push((
            parts.headers["idempotency-key"].to_str().unwrap().into(),
            serde_json::from_slice(&body).unwrap(),
        ));
        if std::mem::take(&mut state.lose_create) {
            json!({})
        } else {
            computer(false)
        }
    } else if parts.method == "POST" && path.ends_with("/commands") {
        let parsed: Value = serde_json::from_slice(&body).unwrap();
        let key = parts.headers["idempotency-key"]
            .to_str()
            .unwrap()
            .to_owned();
        if !state.commands.iter().any(|(old, _)| old == &key) {
            state.effects += 1;
            if state.run_bootstrap {
                assert!(
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(parsed["command"].as_str().unwrap())
                        .status()
                        .unwrap()
                        .success()
                );
            }
        }
        state.commands.push((key, parsed));
        if std::mem::take(&mut state.lose_command) {
            json!({})
        } else {
            command()
        }
    } else if parts.method == "DELETE" && path.starts_with("/v1/computers/") {
        state.deletes += 1;
        let bytes = if std::mem::take(&mut state.lose_delete) {
            "uncertain"
        } else {
            ""
        };
        return Response::builder()
            .status(202)
            .body(Body::from(bytes))
            .unwrap();
    } else if path.contains("/commands/") {
        command()
    } else if path.starts_with("/v1/computers/") {
        let deleted = if state.deletes > 0 {
            state.deleted_reads += 1;
            state.deleted_reads > 1
        } else {
            false
        };
        computer(deleted)
    } else {
        panic!("unexpected route: {path}")
    };
    axum::Json(response).into_response()
}
fn computer(deleted: bool) -> Value {
    json!({"id":ID,"kind":"cloud","size":"small","name":"Lab worker","state":if deleted {"deleted"} else {"awake"},"desired":if deleted {"deleted"} else {"running"},"online":!deleted})
}
fn command() -> Value {
    json!({"id":COMMAND,"computer":ID,"state":"succeeded","exit_code":0,"digest":"a".repeat(64),"cancel_requested":false,"output_limit_bytes":65536,"output_expires_unix":2000000000,"stdout_truncated":false,"stderr_truncated":false})
}
struct Server {
    client: Client,
    session: Session,
    fixture: Fixture,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server() -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let fixture = Fixture(Arc::new(Mutex::new(Seen::default())));
    let app = Router::new().fallback(serve).with_state(fixture.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(Deployment::new(&base, "https://workers.example").unwrap()).unwrap();
    let session = client.authenticate(&account_token()).await.unwrap();
    Server {
        client,
        session,
        fixture,
        task,
    }
}
fn prepare(dir: &Path, guest: &Path, server: &Server) -> Launcher {
    Launcher::prepare(
        dir,
        &server.client,
        &server.session,
        "job-run-1",
        settings(guest),
        &requirements(),
        enrolled(),
        NOW,
    )
    .unwrap()
}
async fn step(launcher: &mut Launcher, server: &Server) -> Progress {
    launcher
        .step(
            &server.client,
            &server.session,
            NOW,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn response_loss_restart_and_uncertain_delete_keep_one_worker_and_cleanup_id() {
    let server = server().await;
    {
        let mut state = server.fixture.0.lock().unwrap();
        state.lose_create = true;
        state.lose_command = true;
        state.lose_delete = true;
        state.run_bootstrap = true;
    }
    let root = tempfile::tempdir().unwrap();
    let guest = tempfile::tempdir().unwrap();
    let directory = root.path().join("run");
    let mut launcher = prepare(&directory, guest.path(), &server);
    assert!(format!("{launcher:?}").find("labw_").is_none());
    assert!(
        !fs::read_to_string(directory.join("state.json"))
            .unwrap()
            .contains(&account_token())
    );
    assert!(
        launcher
            .step(
                &server.client,
                &server.session,
                NOW,
                &CancellationToken::new()
            )
            .await
            .is_err()
    );
    drop(launcher);
    let mut launcher = Launcher::open(&directory).unwrap();
    assert_eq!(step(&mut launcher, &server).await, Progress::Starting);
    let reference = launcher.computer().unwrap().id().to_owned();
    assert_eq!(step(&mut launcher, &server).await, Progress::Starting);
    assert!(
        launcher
            .step(
                &server.client,
                &server.session,
                NOW,
                &CancellationToken::new()
            )
            .await
            .is_err()
    );
    drop(launcher);
    let mut launcher = Launcher::open(&directory).unwrap();
    assert_eq!(step(&mut launcher, &server).await, Progress::Running);
    assert_eq!(step(&mut launcher, &server).await, Progress::Cleaning);
    assert!(
        launcher
            .step(
                &server.client,
                &server.session,
                NOW,
                &CancellationToken::new()
            )
            .await
            .is_err()
    );
    drop(launcher);
    let mut launcher = Launcher::open(&directory).unwrap();
    assert_eq!(launcher.computer().unwrap().id(), reference);
    assert_eq!(step(&mut launcher, &server).await, Progress::Cleaning);
    assert_eq!(
        step(&mut launcher, &server).await,
        Progress::Finished {
            command_state: Some("succeeded".into())
        }
    );
    let state = server.fixture.0.lock().unwrap();
    assert_eq!(state.creates.len(), 2);
    assert_eq!(state.creates[0], state.creates[1]);
    assert_eq!(state.commands.len(), 2);
    assert_eq!(state.commands[0], state.commands[1]);
    assert_eq!(state.effects, 1);
    assert_eq!(state.deletes, 1);
    let config_path = PathBuf::from(fs::read_to_string(guest.path().join("config-path")).unwrap());
    let config: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    let credential_path = PathBuf::from(config["credential"].as_str().unwrap());
    let key: Value = serde_json::from_slice(&fs::read(&credential_path).unwrap()).unwrap();
    assert_eq!(key["token"], "labw_worker_fixture.job_only_fixture");
    assert!(config["local_assets"].as_object().unwrap().is_empty());
    for path in [directory.join("state.json"), config_path, credential_path] {
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        assert!(!fs::read_to_string(path).unwrap().contains(&account_token()));
    }
    assert!(
        !fs::read_to_string(directory.join("state.json"))
            .unwrap()
            .contains("labw_")
    );
}

#[tokio::test]
async fn cleanup_before_create_and_expired_unsent_run_have_no_remote_effect() {
    let server = server().await;
    let root = tempfile::tempdir().unwrap();
    let guest = tempfile::tempdir().unwrap();
    let mut launcher = prepare(&root.path().join("canceled"), guest.path(), &server);
    launcher.request_cleanup().unwrap();
    assert!(matches!(
        step(&mut launcher, &server).await,
        Progress::Finished { .. }
    ));
    let mut expired = prepare(&root.path().join("expired"), guest.path(), &server);
    assert!(matches!(
        expired
            .step(
                &server.client,
                &server.session,
                NOW + 120_000,
                &CancellationToken::new()
            )
            .await
            .unwrap(),
        Progress::Finished { .. }
    ));
    assert!(server.fixture.0.lock().unwrap().creates.is_empty());
}

#[tokio::test]
async fn private_input_wrong_account_revocation_and_unscoped_keys_fail_before_effects() {
    let server = server().await;
    let root = tempfile::tempdir().unwrap();
    let guest = tempfile::tempdir().unwrap();
    for case in 0..7 {
        let mut key = enrolled();
        let mut requirements = requirements();
        match case {
            0 => requirements.private_local = true,
            1 => {
                requirements.local_assets.insert("a".repeat(64));
            }
            2 => key.worker.account = "acct_other".into(),
            3 => key.worker.revoked = true,
            4 => key.worker.issuer_session = None,
            5 => key.worker.expires_unix_ms = NOW + 86_400_001,
            6 => key.token = account_token(),
            _ => unreachable!(),
        }
        assert!(
            Launcher::prepare(
                &root.path().join(case.to_string()),
                &server.client,
                &server.session,
                "run",
                settings(guest.path()),
                &requirements,
                key,
                NOW
            )
            .is_err()
        );
    }
    assert!(server.fixture.0.lock().unwrap().creates.is_empty());
}

#[tokio::test]
async fn concurrent_coordinator_is_refused_and_pinned_worker_is_checked_before_key_write() {
    let server = server().await;
    let root = tempfile::tempdir().unwrap();
    let guest = tempfile::tempdir().unwrap();
    let directory = root.path().join("run");
    let mut launcher = prepare(&directory, guest.path(), &server);
    assert!(Launcher::open(&directory).is_err());
    fs::write(guest.path().join("labs-worker"), "wrong executable").unwrap();
    step(&mut launcher, &server).await;
    step(&mut launcher, &server).await;
    let value: Value =
        serde_json::from_slice(&fs::read(directory.join("state.json")).unwrap()).unwrap();
    let command = value["stage"]["Starting"]["body"]["command"]
        .as_str()
        .unwrap();
    assert!(
        !std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .status()
            .unwrap()
            .success()
    );
    assert!(!guest.path().read_dir().unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".lab-")
    }));
    launcher.request_cleanup().unwrap();
    assert_eq!(step(&mut launcher, &server).await, Progress::Cleaning);
    assert_eq!(server.fixture.0.lock().unwrap().effects, 0);
}
