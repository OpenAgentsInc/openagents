//! The real `boat` SDK against `oa-boat` on loopback, with Compute Engine
//! faked in memory and each "VM" a local shell with its own home.

use boat::models::*;
use boat::{ApiKey, Client, Nullable, WaitOptions};
use oa_boat::api::{Token, router};
use oa_boat::remote::Local;
use oa_boat::{Config, Service};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

mod common;
use common::Fake;

const TOKEN: &str = "test-token-0123456789abcdefghij";

struct Rig {
    client: Client,
    service: Arc<Service<Fake, Local>>,
    fake: Fake,
    _dir: tempfile::TempDir,
}

async fn rig(cfg: impl FnOnce(&mut Config)) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    let mut c = Config::for_tests();
    cfg(&mut c);
    let service = Service::new(
        c,
        fake.clone(),
        Local {
            root: dir.path().to_owned(),
        },
    );
    let app = router(service.clone(), Token::new(TOKEN));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::builder(ApiKey::new(TOKEN).unwrap())
        .base_url(format!("http://127.0.0.1:{port}/api/v1"))
        .build()
        .unwrap();
    Rig {
        client,
        service,
        fake,
        _dir: dir,
    }
}

fn fast() -> WaitOptions {
    WaitOptions {
        timeout: Duration::from_secs(30),
        interval: Duration::from_millis(50),
        ..Default::default()
    }
}

async fn raw_json(client: &Client, path: &str) -> Value {
    reqwest::Client::new()
        .get(format!("{}{path}", client.origin()))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn create(c: &Client, body: CreateSandboxRequest, key: Option<&str>) -> String {
    c.create(&CreateParams {
        idempotency_key: key.map(Into::into),
        body: Some(body),
        ..Default::default()
    })
    .await
    .unwrap()
    .sandbox
    .id
}

async fn run(c: &Client, id: &str, cmd: &str) -> CommandResponse {
    match c
        .command(&CommandParams {
            sandbox_id: id.into(),
            body: CommandRequest {
                command: cmd.into(),
                timeout_seconds: Some(30),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap()
    {
        CommandResponseBody::Finished(r) => r,
        CommandResponseBody::Started(_) => panic!("detached"),
    }
}

#[tokio::test]
async fn a_sandbox_lives_its_whole_life_through_the_sdk() {
    let r = rig(|_| {}).await;
    let c = &r.client;
    let id = create(
        c,
        CreateSandboxRequest {
            type_: Some("small".into()),
            env: Some(BTreeMap::from([("FOO".into(), "it's bar".into())])),
            setup_script: Some("echo set > \"$HOME/setup-ran\"".into()),
            ..Default::default()
        },
        None,
    )
    .await;
    assert!(id.starts_with("bx_"));
    let ready = c.wait_until_ready(&id, &fast()).await.unwrap();
    assert_eq!(ready.state, "ready");
    assert_eq!(ready.type_.as_deref(), Some("small"));
    assert_eq!(ready.hydrated, Some(true));
    assert_eq!(ready.holds_creator_logins, Some(false));

    // Synchronous: output, exit code, env, setup script.
    let done = run(
        c,
        &id,
        "echo hi; echo err >&2; echo \"$FOO\"; cat setup-ran; exit 3",
    )
    .await;
    assert_eq!(done.stdout, "hi\nit's bar\nset\n");
    assert_eq!(done.stderr, "err\n");
    assert_eq!(done.exit_code, Some(3));
    assert!(!done.success && !done.timed_out);

    // Streamed.
    let mut stream = c
        .exec_stream(
            &id,
            CommandRequest {
                command: "printf a; sleep 0.2; printf b; echo oops >&2".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let (mut out, mut err, mut exit) = (String::new(), String::new(), None);
    while let Some(f) = stream.next().await.unwrap() {
        match f {
            boat::CommandFrame::Stdout(s) => out.push_str(&s),
            boat::CommandFrame::Stderr(s) => err.push_str(&s),
            boat::CommandFrame::Exit { exit_code, .. } => exit = exit_code,
            _ => {}
        }
    }
    assert_eq!(
        (out.as_str(), err.as_str(), exit),
        ("ab", "oops\n", Some(0))
    );

    // Files, both encodings, and a missing one.
    c.write_text(&id, "dir/a.txt", "hello").await.unwrap();
    assert_eq!(c.read_text(&id, "dir/a.txt").await.unwrap(), "hello");
    c.write_bytes(&id, "b.bin", &[0, 255, 7]).await.unwrap();
    assert_eq!(c.read_bytes(&id, "b.bin").await.unwrap(), vec![0, 255, 7]);
    let missing = c.read_text(&id, "nope").await.unwrap_err();
    assert!(missing.to_string().contains("404"), "{missing}");

    // Detached, then waited for.
    let started = c
        .exec_detached(
            &id,
            CommandRequest {
                command: "echo out; sleep 0.3; exit 5".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(started.process_id > 1);
    let status = c
        .wait_command(&id, started.process_id, &fast())
        .await
        .unwrap();
    assert_eq!(status.exit_code, Some(5));
    assert_eq!(status.stdout, "out\n");
    assert!(status.log_path.unwrap().ends_with("/out"));

    // Usage reports a rate.
    let usage = c
        .usage(&UsageParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(usage.running);
    assert!(usage.extra["dollarsPerHour"].as_f64().unwrap() > 0.0);
    assert_eq!(usage.sandbox_type, "small");

    // Stop keeps the disk; the latest snapshot names the stop; resume
    // brings it back with fresh env.
    let stop = c
        .stop(&StopParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    let Nullable::Value(sb) = stop.sandbox else {
        panic!()
    };
    let Nullable::Value(op) = sb.stop else {
        panic!()
    };
    let done = c.wait_for_stop(&id, &op.id, &fast()).await.unwrap();
    assert_eq!(done.status, "completed");
    let latest = c
        .get_latest_sandbox_snapshot(&GetLatestSandboxSnapshotParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap()
        .snapshot
        .unwrap();
    assert_eq!(latest.status, "completed");
    assert_eq!(latest.size_bytes, None);
    let wire = raw_json(c, &format!("/sandboxes/{id}/snapshots/latest")).await;
    assert!(
        wire["snapshot"]
            .get("sizeBytes")
            .is_some_and(Value::is_null)
    );
    let refused = c
        .command(&CommandParams {
            sandbox_id: id.clone(),
            body: CommandRequest {
                command: "true".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("409"), "{refused}");

    // A template from the stopped disk.
    let saved = c
        .save_named_snapshot(&SaveNamedSnapshotParams {
            body: NamedSnapshotSaveRequest {
                sandbox_id: id.clone(),
                name: "oa-coder-main-20261010".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(saved.snapshot.source_sandbox_id, id);
    assert_eq!(
        saved.snapshot.extra["providerImage"],
        json!({"id":null,"source":"gce.images.id"})
    );
    let got = c
        .get_named_snapshot(&GetNamedSnapshotParams {
            name: "oa-coder-main-20261010".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(got.snapshot.status, "ready");
    assert!(got.snapshot.snapshot_id.is_some());
    let listed = c.list_named_snapshots().await.unwrap();
    assert_eq!(listed.snapshots.len(), 1);

    c.resume(&ResumeParams {
        sandbox_id: id.clone(),
        body: Some(ResumeRequest {
            env: Some(BTreeMap::from([("FOO".into(), "two".into())])),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await
    .unwrap();
    c.wait_until_ready(&id, &fast()).await.unwrap();
    assert_eq!(
        run(c, &id, "echo $FOO; cat dir/a.txt").await.stdout,
        "two\nhello"
    );

    // A sandbox from the template.
    let child = create(
        c,
        CreateSandboxRequest {
            from_: Some("oa-coder-main-20261010".into()),
            type_: Some("large".into()),
            extra: BTreeMap::from([("provisioning".into(), json!("spot"))]),
            ..Default::default()
        },
        None,
    )
    .await;
    let child_sb = c.wait_until_ready(&child, &fast()).await.unwrap();
    assert_eq!(child_sb.extra["provisioning"], "spot");
    assert_eq!(child_sb.extra["machineType"], "n2d-standard-8");

    // Delete both, and the template.
    for x in [&id, &child] {
        let op = c
            .delete_sandbox(&DeleteSandboxParams {
                sandbox_id: x.clone(),
                x_ascii_confirm_delete: x.clone(),
                ..Default::default()
            })
            .await
            .unwrap()
            .operation;
        let gone = c.wait_for_deletion(&op.id, &fast()).await.unwrap();
        assert_eq!(gone.status, "completed");
    }
    c.delete_named_snapshot(&DeleteNamedSnapshotParams {
        name: "oa-coder-main-20261010".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    assert!(c.list_named_snapshots().await.unwrap().snapshots.is_empty());
    let err = c
        .get(&GetParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("404"), "{err}");
}

#[tokio::test]
async fn image_archive_bytes_stay_distinct_from_restored_content_and_disk_capacity() {
    let r = rig(|_| {}).await;
    let cases = [
        ("archive-reported", Some(1234), Some(1234_u64)),
        ("archive-unknown", None, None),
        ("archive-zero", Some(0), Some(0)),
        ("archive-negative", Some(-1), None),
    ];
    {
        let mut world = r.fake.0.lock().unwrap();
        for (name, archive_bytes, _) in cases {
            world.images.insert(
                name.into(),
                oa_boat::gce::Image {
                    name: name.into(),
                    id: format!("{name}-id"),
                    status: "READY".into(),
                    labels: BTreeMap::from([
                        (
                            oa_boat::service::MANAGED.into(),
                            oa_boat::service::MANAGED_TEMPLATE.into(),
                        ),
                        ("oa-boat-src".into(), "archive-fixture".into()),
                    ]),
                    disk_gb: 200,
                    archive_bytes,
                    ..Default::default()
                },
            );
        }
    }
    for (name, _, observed) in cases {
        let got = r
            .client
            .get_named_snapshot(&GetNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(got.snapshot.size_bytes, None);
        let wire = raw_json(&r.client, &format!("/named-snapshots/{name}")).await;
        let wire = &wire["snapshot"];
        assert!(wire.get("sizeBytes").is_none());
        assert_eq!(
            wire["providerArchiveSize"],
            json!({
                "bytes": observed, "source":"gce.images.archiveSizeBytes"
            })
        );
        assert_eq!(wire["diskGb"], 200);
    }
    let listed = r.client.list_named_snapshots().await.unwrap();
    assert_eq!(listed.snapshots.len(), cases.len());
    for snapshot in listed.snapshots {
        let (_, _, observed) = cases
            .iter()
            .find(|(name, _, _)| *name == snapshot.name)
            .unwrap();
        assert_eq!(snapshot.size_bytes, None);
        assert_eq!(
            snapshot.extra["providerArchiveSize"],
            json!({
                "bytes": observed, "source":"gce.images.archiveSizeBytes"
            })
        );
    }
}

#[tokio::test]
async fn named_snapshot_provider_ids_cover_all_image_states() {
    let r = rig(|_| {}).await;
    let cases = [
        ("ready-id", "READY", "1", Some("1")),
        (
            "pending-id",
            "PENDING",
            "18446744073709551615",
            Some("18446744073709551615"),
        ),
        ("failed-id", "FAILED", "3", Some("3")),
        ("deleting-id", "DELETING", "4", Some("4")),
        ("missing-id", "PENDING", "", None),
        ("invalid-id", "FAILED", "01", None),
        ("overflow-id", "FAILED", "18446744073709551616", None),
    ];
    {
        let mut world = r.fake.0.lock().unwrap();
        for (name, status, id, _) in cases {
            world.images.insert(
                name.into(),
                oa_boat::gce::Image {
                    name: name.into(),
                    id: id.into(),
                    status: status.into(),
                    labels: BTreeMap::from([(
                        oa_boat::service::MANAGED.into(),
                        oa_boat::service::MANAGED_TEMPLATE.into(),
                    )]),
                    ..Default::default()
                },
            );
        }
    }
    let listed = r.client.list_named_snapshots().await.unwrap();
    assert_eq!(listed.snapshots.len(), cases.len());
    for (name, state, id, expected) in cases {
        let got = r
            .client
            .get_named_snapshot(&GetNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .snapshot;
        let listed = listed.snapshots.iter().find(|i| i.name == name).unwrap();
        assert_eq!(
            got.extra["providerImage"],
            json!({"id": expected, "source":"gce.images.id"})
        );
        assert_eq!(listed.extra["providerImage"], got.extra["providerImage"]);
        assert_eq!(got.snapshot_id.as_deref(), (state == "READY").then_some(id));
    }
}

#[tokio::test]
async fn idempotent_creates_full_zones_and_refusals() {
    let r = rig(|_| {}).await;
    r.fake.0.lock().unwrap().full_zones = vec!["us-central1-a".into()];
    let c = &r.client;
    let a = create(c, CreateSandboxRequest::default(), Some("job-1")).await;
    let b = create(c, CreateSandboxRequest::default(), Some("job-1")).await;
    assert_eq!(a, b);
    c.wait_until_ready(&a, &fast()).await.unwrap();
    let zone = r
        .fake
        .0
        .lock()
        .unwrap()
        .instances
        .values()
        .next()
        .unwrap()
        .zone
        .clone();
    assert_eq!(zone, "us-central1-b");

    // Limits answer, the integrated agent does not, a bad key is refused.
    let limits = c.limits(&LimitsParams::default()).await.unwrap();
    assert!(limits.can_start);
    assert_eq!(limits.active_sandboxes, 1);
    let prompt = c
        .prompt(&PromptParams {
            sandbox_id: a.clone(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(prompt.to_string().contains("501"), "{prompt}");
    let bad = Client::builder(ApiKey::new("wrong-wrong-wrong-wrong-wrong").unwrap())
        .base_url(c.origin())
        .build()
        .unwrap();
    let e = bad.limits(&LimitsParams::default()).await.unwrap_err();
    assert!(e.to_string().contains("401"), "{e}");

    // Deleting needs the confirmation header to name the sandbox.
    let e = c
        .delete_sandbox(&DeleteSandboxParams {
            sandbox_id: a.clone(),
            x_ascii_confirm_delete: "bx_other".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(e.to_string().contains("400"), "{e}");
}

#[tokio::test]
async fn a_size_with_no_room_in_any_zone_lands_on_its_alternate_machine() {
    let r = rig(|_| {}).await;
    r.fake.0.lock().unwrap().full_machines = vec!["n2d-standard-8".into()];
    let c = &r.client;
    let id = create(
        c,
        CreateSandboxRequest {
            type_: Some("large".into()),
            ..Default::default()
        },
        None,
    )
    .await;
    let ready = c.wait_until_ready(&id, &fast()).await.unwrap();
    assert_eq!(ready.type_.as_deref(), Some("large"));
    assert_eq!(ready.extra["machineType"], json!("e2-standard-8"));
    assert_eq!(ready.extra["dollarsPerHour"], json!(0.26805));
    assert!(ready.extra.get("progress").is_none());

    // The alternate full too: the sandbox fails in plain words.
    r.fake.0.lock().unwrap().full_machines = vec!["n2d-standard-8".into(), "e2-standard-8".into()];
    let id = create(
        c,
        CreateSandboxRequest {
            type_: Some("large".into()),
            ..Default::default()
        },
        None,
    )
    .await;
    c.wait_until_ready(&id, &fast()).await.unwrap_err();
    let failed = c
        .get(&GetParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap()
        .sandbox;
    assert_eq!(failed.state, "error");
    assert_eq!(
        failed.error,
        Nullable::Value("Google has no large computer free in any zone right now.".into())
    );
}

#[tokio::test]
async fn the_reaper_stops_on_ttl_and_on_idle_but_not_while_busy() {
    let r = rig(|c| c.default_idle = 1).await;
    let c = &r.client;
    let ttl = create(
        c,
        CreateSandboxRequest {
            ttl_seconds: Nullable::Value(1),
            extra: BTreeMap::from([("idleStopSeconds".into(), json!(0))]),
            ..Default::default()
        },
        None,
    )
    .await;
    let idle = create(c, CreateSandboxRequest::default(), None).await;
    let busy = create(c, CreateSandboxRequest::default(), None).await;
    for x in [&ttl, &idle, &busy] {
        c.wait_until_ready(x, &fast()).await.unwrap();
    }
    c.exec_detached(
        &busy,
        CommandRequest {
            command: "sleep 30".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let mut stopped = r.service.reap().await;
    stopped.sort();
    let mut want = vec![(idle.clone(), "idle"), (ttl.clone(), "ttl")];
    want.sort();
    assert_eq!(stopped, want);
    // Accounting runs on the next pass and counts each run once.
    r.service.reap().await;
    r.service.reap().await;
    let u = c
        .usage(&UsageParams {
            sandbox_id: idle.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!u.running);
    assert!(u.seconds >= 1 && u.seconds < 60, "{}", u.seconds);
    let _ = c
        .command(&CommandParams {
            sandbox_id: busy.clone(),
            body: CommandRequest {
                command: "pkill -f 'sleep 30' || true".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .await;
}

#[tokio::test]
async fn a_restarted_service_finds_its_ready_sandboxes_at_once() {
    let r = rig(|_| {}).await;
    let id = create(&r.client, CreateSandboxRequest::default(), None).await;
    r.client.wait_until_ready(&id, &fast()).await.unwrap();
    // A second service over the same GCE and the same VMs, with no memory.
    let again = Service::new(
        Config::for_tests(),
        r.fake.clone(),
        Local {
            root: r._dir.path().to_owned(),
        },
    );
    let app = router(again, Token::new(TOKEN));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let c = Client::builder(ApiKey::new(TOKEN).unwrap())
        .base_url(format!("http://127.0.0.1:{port}/api/v1"))
        .build()
        .unwrap();
    // No "still starting" in between: the first command runs.
    assert_eq!(run(&c, &id, "echo again").await.stdout, "again\n");
    let got = c
        .get(&GetParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(got.sandbox.state, "ready");
}

#[tokio::test]
async fn a_lost_insert_wait_tries_the_next_zone_and_a_made_vm_is_used() {
    let r = rig(|_| {}).await;
    {
        let mut w = r.fake.0.lock().unwrap();
        w.lost_zones = vec!["us-central1-a".into()];
    }
    let a = create(&r.client, CreateSandboxRequest::default(), None).await;
    r.client.wait_until_ready(&a, &fast()).await.unwrap();
    {
        let mut w = r.fake.0.lock().unwrap();
        let zones: Vec<_> = w.instances.values().map(|i| i.zone.clone()).collect();
        assert_eq!(zones, vec!["us-central1-b".to_owned()]);
        w.lost_zones.clear();
        w.flaky_zones = vec!["us-central1-a".into()];
    }
    let b = create(&r.client, CreateSandboxRequest::default(), None).await;
    r.client.wait_until_ready(&b, &fast()).await.unwrap();
    let w = r.fake.0.lock().unwrap();
    assert_eq!(w.instances.len(), 2, "no extra VM in another zone");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn protected_workspace_files_are_binary_bounded_and_do_not_wake() {
    use computer_files::{MAX_BYTES, digest};
    let helper = std::env::var_os("OA_COMPUTER_FILES_HELPER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("computer-files")
        });
    assert!(
        helper.is_file(),
        "build the computer-files binary before running protected-file tests"
    );
    let r = rig(|cfg| cfg.workspace_helper = Some(helper)).await;
    let id = create(&r.client, CreateSandboxRequest::default(), None).await;
    r.client.wait_until_ready(&id, &fast()).await.unwrap();
    let caps = r.client.capabilities().await.unwrap();
    assert_eq!(caps["workspace_file_bytes"], MAX_BYTES);
    let bytes: Vec<u8> = (0..=255).cycle().take(100_003).collect();
    let info = r
        .client
        .write_workspace_file(&id, "input.bin", &bytes, false)
        .await
        .unwrap();
    assert_eq!(info.sha256, digest(&bytes));
    assert_eq!(info.size, bytes.len());
    assert_eq!(
        r.client
            .read_workspace_file(&id, "input.bin")
            .await
            .unwrap(),
        bytes
    );
    r.client
        .write_workspace_file(&id, "input.bin", &bytes, false)
        .await
        .unwrap();
    let e = r
        .client
        .write_workspace_file(&id, "input.bin", b"different", false)
        .await
        .unwrap_err();
    assert!(matches!(e,boat::Error::Api(ref e) if e.status.as_u16()==412));
    r.client
        .write_workspace_file(&id, "input.bin", b"replaced", true)
        .await
        .unwrap();
    assert_eq!(
        r.client
            .read_workspace_file(&id, "input.bin")
            .await
            .unwrap(),
        b"replaced"
    );
    run(
        &r.client,
        &id,
        "ln -s /etc workspace/escape; mkfifo workspace/pipe",
    )
    .await;
    for (path, status) in [("../outside", 400), ("escape/passwd", 403), ("pipe", 415)] {
        let e = r.client.read_workspace_file(&id, path).await.unwrap_err();
        assert!(
            matches!(e,boat::Error::Api(ref e) if e.status.as_u16()==status),
            "{e}"
        );
    }
    let full = vec![0x9a; MAX_BYTES];
    r.client
        .write_workspace_file(&id, "full", &full, false)
        .await
        .unwrap();
    assert_eq!(
        r.client.read_workspace_file(&id, "full").await.unwrap(),
        full
    );
    let permit = r
        .service
        .workspace_slots
        .clone()
        .acquire_many_owned(2)
        .await
        .unwrap();
    let e = r
        .client
        .read_workspace_file(&id, "input.bin")
        .await
        .unwrap_err();
    assert!(matches!(e,boat::Error::Api(ref e) if e.status.as_u16()==429));
    drop(permit);
    r.client
        .stop(&StopParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    let e = r
        .client
        .read_workspace_file(&id, "input.bin")
        .await
        .unwrap_err();
    assert!(matches!(e,boat::Error::Api(ref e) if e.status.as_u16()==409));
    r.client
        .resume(&ResumeParams {
            sandbox_id: id.clone(),
            body: Some(ResumeRequest::default()),
            ..Default::default()
        })
        .await
        .unwrap();
    r.client.wait_until_ready(&id, &fast()).await.unwrap();
    assert_eq!(
        r.client
            .read_workspace_file(&id, "input.bin")
            .await
            .unwrap(),
        b"replaced"
    );
    // The unrestricted legacy file contract remains a separate operation.
    r.client
        .write_bytes(&id, "legacy.bin", &bytes)
        .await
        .unwrap();
    assert_eq!(r.client.read_bytes(&id, "legacy.bin").await.unwrap(), bytes);
    r.client
        .delete_sandbox(&DeleteSandboxParams {
            sandbox_id: id.clone(),
            x_ascii_confirm_delete: id,
            ..Default::default()
        })
        .await
        .unwrap();
}
