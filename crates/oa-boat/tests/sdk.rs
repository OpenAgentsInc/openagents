//! The real `boat` SDK against `oa-boat` on loopback, with Compute Engine
//! faked in memory and each "VM" a local shell with its own home.

use boat::models::*;
use boat::{ApiKey, Client, Nullable, WaitOptions};
use oa_boat::api::{Token, router};
use oa_boat::gce::{Compute, GceError, Image, Instance, Result};
use oa_boat::remote::Local;
use oa_boat::{Config, Service, time};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "test-token-0123456789abcdefghij";

#[derive(Default)]
struct World {
    instances: BTreeMap<String, Instance>,
    images: BTreeMap<String, Image>,
    clock: i64,
    full_zones: Vec<String>,
    /// The insert is made but its wait fails in transport.
    flaky_zones: Vec<String>,
    /// The wait fails in transport and the VM never appears.
    lost_zones: Vec<String>,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<World>>);

impl Fake {
    fn tick(w: &mut World) -> String {
        w.clock = w.clock.max(time::now()) + 1;
        time::format(w.clock)
    }
}

fn labels(v: &Value) -> BTreeMap<String, String> {
    v.as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

impl Compute for Fake {
    async fn insert_instance(&self, zone: &str, body: Value) -> Result<()> {
        let mut w = self.0.lock().unwrap();
        if w.full_zones.iter().any(|z| z == zone) {
            return Err(GceError::new(409, "ZONE_RESOURCE_POOL_EXHAUSTED", "full"));
        }
        if w.lost_zones.iter().any(|z| z == zone) {
            return Err(GceError::new(0, "transport", "unreachable"));
        }
        let flaky = w.flaky_zones.iter().any(|z| z == zone);
        let name = body["name"].as_str().unwrap().to_owned();
        let now = Self::tick(&mut w);
        let disk_gb = body["disks"][0]["initializeParams"]["diskSizeGb"]
            .as_str()
            .and_then(|s| s.parse().ok());
        let i = Instance {
            name: name.clone(),
            zone: zone.into(),
            status: "RUNNING".into(),
            labels: labels(&body["labels"]),
            label_fingerprint: "f0".into(),
            ip: Some(name.clone()),
            machine: body["machineType"]
                .as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .into(),
            creation: Some(now.clone()),
            last_start: Some(now),
            last_stop: None,
            disk: Some(name.clone()),
            disk_gb,
            spot: body["scheduling"]["provisioningModel"] == "SPOT",
        };
        w.instances.insert(name, i);
        if flaky {
            return Err(GceError::new(0, "transport", "unreachable"));
        }
        Ok(())
    }
    async fn get_instance(&self, _zone: &str, name: &str) -> Result<Option<Instance>> {
        Ok(self.0.lock().unwrap().instances.get(name).cloned())
    }
    async fn list_instances(&self, label: &str, value: &str) -> Result<Vec<Instance>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .instances
            .values()
            .filter(|i| i.labels.get(label).map(String::as_str) == Some(value))
            .cloned()
            .collect())
    }
    async fn instance_action(&self, _zone: &str, name: &str, action: &str) -> Result<()> {
        let mut w = self.0.lock().unwrap();
        let now = Self::tick(&mut w);
        match action {
            "delete" => {
                w.instances.remove(name);
            }
            "stop" => {
                let i = w.instances.get_mut(name).unwrap();
                i.status = "TERMINATED".into();
                i.last_stop = Some(now);
            }
            "start" => {
                let i = w.instances.get_mut(name).unwrap();
                i.status = "RUNNING".into();
                i.last_start = Some(now);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    async fn set_labels(
        &self,
        _zone: &str,
        name: &str,
        labels: &BTreeMap<String, String>,
        fingerprint: &str,
    ) -> Result<()> {
        let mut w = self.0.lock().unwrap();
        let i = w.instances.get_mut(name).unwrap();
        if i.label_fingerprint != fingerprint {
            return Err(GceError::new(412, "conditionNotMet", "stale"));
        }
        i.labels = labels.clone();
        i.label_fingerprint = format!("{}x", i.label_fingerprint);
        Ok(())
    }
    async fn set_ssh_keys(&self, _zone: &str, _name: &str, _value: &str) -> Result<()> {
        Ok(())
    }
    async fn get_image(&self, name: &str) -> Result<Option<Image>> {
        Ok(self.0.lock().unwrap().images.get(name).cloned())
    }
    async fn image_from_family(&self, family: &str) -> Result<Option<Image>> {
        Ok(Some(Image {
            name: format!("{family}-20261010"),
            id: "1".into(),
            status: "READY".into(),
            disk_gb: 200,
            self_link: format!("projects/test/global/images/{family}-20261010"),
            ..Default::default()
        }))
    }
    async fn list_images(&self, label: &str, value: &str) -> Result<Vec<Image>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .images
            .values()
            .filter(|i| i.labels.get(label).map(String::as_str) == Some(value))
            .cloned()
            .collect())
    }
    async fn insert_image(&self, body: Value) -> Result<()> {
        let mut w = self.0.lock().unwrap();
        let now = Self::tick(&mut w);
        let name = body["name"].as_str().unwrap().to_owned();
        let id = format!("{}", w.clock);
        w.images.insert(
            name.clone(),
            Image {
                name: name.clone(),
                id,
                status: "READY".into(),
                labels: labels(&body["labels"]),
                disk_gb: 200,
                archive_bytes: Some(1234),
                creation: Some(now),
                self_link: format!("projects/test/global/images/{name}"),
                family: None,
            },
        );
        Ok(())
    }
    async fn delete_image(&self, name: &str) -> Result<()> {
        self.0.lock().unwrap().images.remove(name);
        Ok(())
    }
    async fn snapshot_disk(&self, _zone: &str, _disk: &str, _snapshot: &str) -> Result<()> {
        Ok(())
    }
    async fn delete_snapshot(&self, _name: &str) -> Result<()> {
        Ok(())
    }
}

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
