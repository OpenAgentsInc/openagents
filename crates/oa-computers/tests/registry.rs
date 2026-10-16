//! Deployment dispatch against one disposable PostgreSQL database and two
//! independent simulated providers with deliberately overlapping resource IDs.
use computer_driver::{
    Artifact, ArtifactKind, Capabilities, CommandCursor, CommandRead, CommandSpec, CreateRequest,
    Driver, Exec, ExecOutput, Meter, Observation, Outcome, Profile, Shape,
    fake::{FakeDriver, Inject},
};
use oa_computers::{
    Db, Store,
    reconcile::{Reconciler, Settings},
    registry::{Catalog, CreateIntent, Drivers, Needs, Placement, Registry, Target},
    store::Desired,
    turns::Secrets,
};
use serde_json::{Value, json};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

const TOKEN: &str = "registry-fixture-service-token";

#[derive(Clone)]
struct Simulated {
    fake: FakeDriver,
    caps: Capabilities,
    creates: Arc<Mutex<Vec<CreateRequest>>>,
}
impl Simulated {
    fn new(root: &std::path::Path, space: bool) -> Self {
        let mut caps = Capabilities::new(
            if space { "fake-space" } else { "fake-boat" },
            if space {
                vec![Profile::Core, Profile::Suspend]
            } else {
                Profile::ALL.to_vec()
            },
            &["small"],
        );
        caps.command_sessions = true;
        caps.workspace_file_bytes = Some(4096);
        caps.shapes.insert(
            "small".into(),
            Shape {
                vcpus: 2,
                memory_mib: if space { 2048 } else { 8192 },
                min_disk_gib: None,
            },
        );
        caps.default_artifact = Some(Artifact {
            kind: if space {
                ArtifactKind::SpaceTemplate
            } else {
                ArtifactKind::GceImage
            },
            reference: if space {
                format!("fixture/template@sha256:{}", "a".repeat(64))
            } else {
                "projects/fixture/global/images/base".into()
            },
            provider_identity: None,
            disk_gib: None,
        });
        Self {
            fake: FakeDriver::new(root),
            caps,
            creates: Default::default(),
        }
    }
    fn target(&self, id: &str) -> Target {
        Target::new(
            id,
            &format!("https://{id}.fixture.invalid/api"),
            self.caps.clone(),
            self.caps.default_artifact.as_ref().unwrap().kind,
            if self.caps.driver == "fake-space" {
                Placement::FastSandbox
            } else {
                Placement::OurCloud
            },
        )
        .unwrap()
    }
}
macro_rules! forward {
    ($($name:ident($($arg:ident:$ty:ty),*)->$out:ty;)+)=>{$(
        async fn $name(&self,$($arg:$ty),*)->$out {self.fake.$name($($arg),*).await}
    )+};
}
impl Driver for Simulated {
    fn capabilities(&self) -> Capabilities {
        self.caps.clone()
    }
    async fn create(&self, request: &CreateRequest) -> Outcome<String> {
        self.creates.lock().unwrap().push(request.clone());
        let mut request = request.clone();
        if self
            .caps
            .default_artifact
            .as_ref()
            .is_some_and(|artifact| Some(&artifact.reference) == request.image.as_ref())
        {
            // The fixture base image starts an empty directory. Preserve the
            // actual request above so image pinning and retry bytes are checked.
            request.image = None;
        }
        self.fake.create(&request).await
    }
    async fn read_workspace_file(
        &self,
        computer: &str,
        path: &str,
    ) -> Result<Vec<u8>, computer_files::FileError> {
        match self.fake.get_file(computer, path).await {
            Outcome::Done { value } => Ok(value),
            _ => Err(computer_files::FileError::Missing),
        }
    }
    async fn write_workspace_file(
        &self,
        computer: &str,
        path: &str,
        bytes: &[u8],
        overwrite: bool,
    ) -> Result<computer_files::FileInfo, computer_files::FileError> {
        if !overwrite
            && matches!(
                self.fake.get_file(computer, path).await,
                Outcome::Done { .. }
            )
        {
            return Err(computer_files::FileError::Exists);
        }
        match self.fake.put_file(computer, path, bytes).await {
            Outcome::Done { .. } => Ok(computer_files::FileInfo {
                size: bytes.len(),
                sha256: computer_files::digest(bytes),
            }),
            _ => Err(computer_files::FileError::WriteUncertain),
        }
    }
    forward! {
        observe(computer:&str)->Outcome<Observation>;
        exec(computer:&str,exec:&Exec)->Outcome<ExecOutput>;
        start_command(computer:&str,spec:&CommandSpec)->Outcome<String>;
        read_command(computer:&str,id:&str,cursor:CommandCursor,max_bytes:u64)->Outcome<CommandRead>;
        stop_command(computer:&str,id:&str)->Outcome<String>;
        put_file(computer:&str,path:&str,bytes:&[u8])->Outcome<()>;
        get_file(computer:&str,path:&str)->Outcome<Vec<u8>>;
        meter(computer:&str)->Outcome<Meter>;
        delete(computer:&str)->Outcome<()>;
        suspend(computer:&str)->Outcome<String>;
        resume(computer:&str)->Outcome<String>;
        snapshot(computer:&str,name:&str)->Outcome<String>;
        snapshot_ready(name:&str)->Outcome<bool>;
        delete_snapshot(name:&str)->Outcome<()>;
        fork(computer:&str,operation:&str)->Outcome<String>;
    }
}

fn configured(default: &str, a: &Simulated, b: &Simulated) -> Registry<Simulated> {
    Registry::new(
        Catalog::new(default, "test-policy-v1", [a.target("a"), b.target("b")]).unwrap(),
        [("a".into(), a.clone()), ("b".into(), b.clone())].into(),
    )
    .unwrap()
}
fn settings() -> Settings {
    let mut settings = Settings::new("registry-test");
    settings.lease = Duration::from_secs(10);
    settings.revisit = Duration::from_millis(1);
    settings.backoff = Duration::from_millis(1);
    settings.driver_poll = Duration::from_millis(1);
    settings.driver_deadline = Duration::from_secs(5);
    settings
}
async fn visit(db: &Db, store: &Store, driver: Registry<Simulated>, id: &str) {
    // Drive only this row, while all of the providers share the same database.
    // This makes a wrong-provider effect observable without a background tick.
    db.client().await.unwrap().execute("UPDATE computers.computers SET next_attempt_at=now()+interval '1 hour' WHERE id<>$1 AND NOT done",&[&id]).await.unwrap();
    store.poke(id).await.unwrap();
    let settings = settings();
    let entry = store
        .claim(&settings.holder, settings.lease)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry.id, id);
    Rc::new(Reconciler {
        store: store.clone(),
        driver,
        settings,
        turns: Secrets::default(),
    })
    .visit(entry)
    .await;
}
async fn phase(
    db: &Db,
    store: &Store,
    driver: &Registry<Simulated>,
    id: &str,
    wanted: &str,
) -> oa_computers::store::Entry {
    for _ in 0..100 {
        let entry = store.get("alice", id).await.unwrap().unwrap();
        if entry.phase == wanted {
            return entry;
        }
        visit(db, store, driver.clone(), id).await;
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    panic!("computer did not reach {wanted}");
}
async fn server(
    store: Store,
    driver: &Registry<Simulated>,
) -> (String, tokio::task::JoinHandle<()>) {
    let files = oa_computers::files::spawn(store.clone(), driver.clone());
    let app = oa_computers::app_with_registry(
        store,
        driver.catalog().clone(),
        TOKEN,
        Secrets::default(),
        Some(files),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, task)
}
async fn post(url: &str, owner: &str, body: Value) -> (u16, Value) {
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(TOKEN)
        .header("X-OpenAgents-Account", owner)
        .json(&body)
        .send()
        .await
        .unwrap();
    (response.status().as_u16(), response.json().await.unwrap())
}
async fn upload(url: &str, bytes: &[u8]) -> u16 {
    reqwest::Client::new()
        .put(url)
        .bearer_auth(TOKEN)
        .header("X-OpenAgents-Account", "alice")
        .query(&[("path", "same.bin")])
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", bytes.len())
        .header("X-Content-SHA256", computer_files::digest(bytes))
        .body(bytes.to_vec())
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}
async fn download(url: &str, owner: &str) -> (u16, Vec<u8>) {
    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(TOKEN)
        .header("X-OpenAgents-Account", owner)
        .query(&[("path", "same.bin")])
        .send()
        .await
        .unwrap();
    (
        response.status().as_u16(),
        response.bytes().await.unwrap().to_vec(),
    )
}
async fn database() -> Option<Db> {
    let dsn = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL").ok()?;
    Some(Db::scratch(&dsn).await.unwrap().0)
}

#[tokio::test(flavor = "multi_thread")]
async fn one_database_dispatches_overlapping_resources_commands_files_and_cleanup_by_binding() {
    let Some(db) = database().await else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    tokio::task::LocalSet::new().run_until(async {
        let store=Store::new(db.clone());let roots=[tempfile::tempdir().unwrap(),tempfile::tempdir().unwrap()];
        let a=Simulated::new(roots[0].path(),false);let b=Simulated::new(roots[1].path(),true);
        let original=configured("a",&a,&b);let request=CreateIntent {idempotency_key:Some("create-a".into()),..Default::default()};
        let (first,new)=store.create_placed("alice",&request,original.catalog()).await.unwrap();assert!(new);
        let first=phase(&db,&store,&original,&first.id,"awake").await;
        let second_request=CreateIntent {idempotency_key:Some("create-b".into()),placement:Some(Placement::FastSandbox),..Default::default()};
        let (second,_)=store.create_placed("alice",&second_request,original.catalog()).await.unwrap();
        let second=phase(&db,&store,&original,&second.id,"awake").await;
        assert_eq!(first.record.resource(),second.record.resource(),"raw provider IDs deliberately overlap");
        assert_ne!(first.binding.as_ref().unwrap().deployment,second.binding.as_ref().unwrap().deployment);
        assert!(db.client().await.unwrap().execute("UPDATE computers.computers SET binding=NULL WHERE id=$1",&[&first.id]).await.is_err());
        assert_eq!(db.client().await.unwrap().execute("UPDATE computers.computers SET lease_holder='old-worker',lease_until=now()+interval '1 hour' WHERE id=$1",&[&first.id]).await.unwrap(),0);
        let mut a_new_defaults=a.clone();
        a_new_defaults.caps.default_artifact.as_mut().unwrap().reference="projects/fixture/global/images/new-default".into();
        let restarted=configured("b",&a_new_defaults,&b);
        let (same,new)=store.create_placed("alice",&request,restarted.catalog()).await.unwrap();assert!(!new);assert_eq!(same.id,first.id);
        assert_eq!(same.binding.as_ref().unwrap().deployment,"a");
        assert_eq!(serde_json::to_value(&same.binding).unwrap(),serde_json::to_value(&first.binding).unwrap());
        let bound=restarted.resolve(&same).unwrap();
        let before=a.fake.calls().len();
        assert!(matches!(bound.create(&CreateRequest{operation:"wrong-image".into(),size:Some("small".into()),image:Some("projects/fixture/global/images/new-default".into()),ttl_seconds:None,labels:Default::default()}).await,Outcome::Failed{..}));
        assert_eq!(a.fake.calls().len(),before);
        let (base,task)=server(store.clone(),&restarted).await;
        let a_files=format!("{base}/v1/computers/{}/files",first.id);let b_files=format!("{base}/v1/computers/{}/files",second.id);
        assert_eq!(upload(&a_files,b"alpha\0\xff").await,200);assert_eq!(upload(&b_files,b"beta\0\x80").await,200);
        assert_eq!(download(&a_files,"alice").await,(200,b"alpha\0\xff".to_vec()));
        assert_eq!(download(&b_files,"alice").await,(200,b"beta\0\x80".to_vec()));
        assert_eq!(download(&a_files,"bob").await.0,404);
        a.fake.inject("start_command",Inject::LoseReply);
        for (id,word) in [(&first.id,"alpha"),(&second.id,"beta")] {
            let url=format!("{base}/v1/computers/{id}/commands");let command=json!({"command":format!("printf {word}"),"idempotency_key":format!("command-{word}")});
            let (status,created)=post(&url,"alice",command.clone()).await;assert_eq!(status,202,"{created}");
            let (status,retry)=post(&url,"alice",command).await;assert_eq!(status,200);assert_eq!(created["id"],retry["id"]);
            for _ in 0..100 {
                let response=reqwest::Client::new().get(format!("{url}/{}",created["id"].as_str().unwrap())).bearer_auth(TOKEN).header("X-OpenAgents-Account","alice").send().await.unwrap().json::<Value>().await.unwrap();
                if response["state"]=="succeeded" {assert_eq!(response["exit_code"],0);break;}
                visit(&db,&store,restarted.clone(),id).await;tokio::time::sleep(Duration::from_millis(3)).await;
            }
            let output=reqwest::Client::new().get(format!("{url}/{}/output",created["id"].as_str().unwrap())).bearer_auth(TOKEN).header("X-OpenAgents-Account","alice").send().await.unwrap().json::<Value>().await.unwrap();
            use base64::Engine;assert_eq!(base64::engine::general_purpose::STANDARD.decode(output["stdout"].as_str().unwrap()).unwrap(),word.as_bytes());
        }
        for provider in [&a,&b] {assert_eq!(provider.fake.calls().iter().filter(|call|call.starts_with("start_command:")).count(),1);}
        let command=json!({"command":"printf alpha","idempotency_key":"command-alpha"});
        let (_,known)=post(&format!("{base}/v1/computers/{}/commands",first.id),"alice",command.clone()).await;
        let missing=Registry::new(Catalog::new("b","missing",[b.target("b")]).unwrap(),[("b".into(),b.clone())].into()).unwrap();
        let (missing_base,missing_task)=server(store.clone(),&missing).await;
        let (status,retried)=post(&format!("{missing_base}/v1/computers/{}/commands",first.id),"alice",command).await;
        assert_eq!(status,200);assert_eq!(retried["id"],known["id"]);
        assert_eq!(post(&format!("{missing_base}/v1/computers/{}/commands",first.id),"alice",json!({"command":"false","idempotency_key":"command-alpha"})).await.0,409);
        missing_task.abort();
        for id in [&first.id,&second.id] {
            store.set_desired("alice",id,Desired::Stopped).await.unwrap();phase(&db,&store,&restarted,id,"stopped").await;
            if id==&first.id {
                let current=store.get("alice",id).await.unwrap().unwrap();
                let bound=restarted.resolve(&current).unwrap();
                let Outcome::Done{value:snapshot}=bound.snapshot(current.record.resource().unwrap(),"same-snapshot-name").await else {panic!("snapshot failed")};
                assert!(snapshot.starts_with("oa-"));
                assert!(matches!(bound.snapshot_ready(&snapshot).await,Outcome::Done{value:true}));
                assert!(matches!(bound.delete_snapshot(&snapshot).await,Outcome::Done{..}));
                let Outcome::Done{value:fork}=bound.fork(current.record.resource().unwrap(),"same-fork-operation").await else {panic!("fork failed")};
                assert!(matches!(bound.fork(current.record.resource().unwrap(),"same-fork-operation").await,Outcome::Done{value} if value==fork));
                assert!(matches!(bound.delete(&fork).await,Outcome::Done{..}));
                assert!(matches!(bound.meter(current.record.resource().unwrap()).await,Outcome::Done{..}));
            }
            store.set_desired("alice",id,Desired::Running).await.unwrap();phase(&db,&store,&restarted,id,"awake").await;
        }
        assert_eq!(download(&a_files,"alice").await.1,b"alpha\0\xff");assert_eq!(download(&b_files,"alice").await.1,b"beta\0\x80");
        for id in [&first.id,&second.id] {store.set_desired("alice",id,Desired::Deleted).await.unwrap();phase(&db,&store,&restarted,id,"deleted").await;}
        assert!(a.fake.live().is_empty()&&b.fake.live().is_empty());task.abort();
    }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_or_changed_deployment_cannot_delete_or_read_another_providers_same_id() {
    let Some(db) = database().await else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async {
            let store = Store::new(db.clone());
            let roots = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
            let a = Simulated::new(roots[0].path(), false);
            let b = Simulated::new(roots[1].path(), true);
            let registry = configured("a", &a, &b);
            let request = CreateIntent {
                idempotency_key: Some("original".into()),
                ..Default::default()
            };
            let (first, _) = store
                .create_placed("alice", &request, registry.catalog())
                .await
                .unwrap();
            phase(&db, &store, &registry, &first.id, "awake").await;
            let (second, _) = store
                .create_placed(
                    "alice",
                    &CreateIntent {
                        placement: Some(Placement::FastSandbox),
                        ..Default::default()
                    },
                    registry.catalog(),
                )
                .await
                .unwrap();
            phase(&db, &store, &registry, &second.id, "awake").await;
            let missing = Registry::new(
                Catalog::new("b", "p2", [b.target("b")]).unwrap(),
                [("b".into(), b.clone())].into(),
            )
            .unwrap();
            let (retry, new) = store
                .create_placed("alice", &request, missing.catalog())
                .await
                .unwrap();
            assert!(!new && retry.id == first.id);
            let (base, task) = server(store.clone(), &missing).await;
            let before = b.fake.calls().len();
            assert_eq!(
                download(&format!("{base}/v1/computers/{}/files", first.id), "alice")
                    .await
                    .0,
                503
            );
            assert_eq!(
                post(
                    &format!("{base}/v1/computers/{}/commands", first.id),
                    "alice",
                    json!({"command":"true","idempotency_key":"must-not-queue"})
                )
                .await
                .0,
                503
            );
            let count: i64 = db
                .client()
                .await
                .unwrap()
                .query_one(
                    "SELECT count(*) FROM computers.commands WHERE computer=$1",
                    &[&first.id],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(count, 0);
            store
                .set_desired("alice", &first.id, Desired::Deleted)
                .await
                .unwrap();
            visit(&db, &store, missing, &first.id).await;
            assert_eq!(b.fake.calls().len(), before);
            assert_eq!(a.fake.live().len(), 1);
            assert_eq!(b.fake.live().len(), 1);
            let held = store.get("alice", &first.id).await.unwrap().unwrap();
            assert_ne!(held.phase, "deleted");
            assert!(held.last_error.is_some());
            let mut changed = a.target("a");
            changed.fingerprint = "f".repeat(64);
            let changed = Registry::new(
                Catalog::new("a", "p3", [changed, b.target("b")]).unwrap(),
                [("a".into(), a.clone()), ("b".into(), b.clone())].into(),
            )
            .unwrap();
            let before = a.fake.calls().len();
            visit(&db, &store, changed, &first.id).await;
            assert_eq!(a.fake.calls().len(), before);
            for id in [&first.id, &second.id] {
                store
                    .set_desired("alice", id, Desired::Deleted)
                    .await
                    .unwrap();
                phase(&db, &store, &registry, id, "deleted").await;
            }
            assert!(a.fake.live().is_empty() && b.fake.live().is_empty());
            task.abort();
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn legacy_unknown_create_keeps_original_operation_and_image_omission_after_explicit_backfill()
{
    let Some(db) = database().await else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async {
            let store = Store::new(db.clone());
            let root = tempfile::tempdir().unwrap();
            let a = Simulated::new(root.path(), false);
            let body = oa_computers::api::CreateBody {
                idempotency_key: Some("legacy".into()),
                ..Default::default()
            };
            let record = oa_computers::api::admit_create(&a.caps, "alice", &body).unwrap();
            let (entry, _) = store
                .create(&oa_computers::store::NewComputer {
                    owner: "alice",
                    chat: None,
                    idempotency_key: Some("legacy"),
                    driver: &a.caps.driver,
                    image: None,
                    record: &record,
                })
                .await
                .unwrap();
            a.fake.inject("create", Inject::LoseReply);
            let settings = settings();
            store.poke(&entry.id).await.unwrap();
            let claimed = store
                .claim(&settings.holder, settings.lease)
                .await
                .unwrap()
                .unwrap();
            Rc::new(Reconciler {
                store: store.clone(),
                driver: a.clone(),
                settings,
                turns: Secrets::default(),
            })
            .visit(claimed)
            .await;
            assert_eq!(a.fake.live().len(), 1);
            let mut target = a.target("a");
            target.legacy_driver = Some(a.caps.driver.clone());
            let registry = Registry::new(
                Catalog::new("a", "new-policy", [target]).unwrap(),
                [("a".into(), a.clone())].into(),
            )
            .unwrap();
            assert_eq!(store.pin_legacy(registry.catalog()).await.unwrap(), 1);
            let saved = store.get("alice", &entry.id).await.unwrap().unwrap();
            assert!(saved.binding.as_ref().unwrap().migrated);
            assert!(
                saved.image.is_none()
                    && saved
                        .binding
                        .as_ref()
                        .unwrap()
                        .operation_namespace
                        .is_none()
            );
            phase(&db, &store, &registry, &entry.id, "awake").await;
            let creates = a.creates.lock().unwrap();
            assert!(!creates.is_empty());
            assert!(creates.iter().all(
                |request| request.operation == creates[0].operation && request.image.is_none()
            ));
            drop(creates);
            assert_eq!(a.fake.live().len(), 1);
            store
                .set_desired("alice", &entry.id, Desired::Deleted)
                .await
                .unwrap();
            phase(&db, &store, &registry, &entry.id, "deleted").await;
            assert!(a.fake.live().is_empty());
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn incompatible_requests_and_command_profiles_refuse_without_durable_effects() {
    let Some(db) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async {
            let store = Store::new(db.clone());
            let roots = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
            let a = Simulated::new(roots[0].path(), false);
            let mut b = Simulated::new(roots[1].path(), true);
            b.caps.command_sessions = false;
            let mut target = b.target("b");
            target.rollout_accounts = Some(["alice".into()].into());
            let registry = Registry::new(
                Catalog::new("a", "p1", [a.target("a"), target]).unwrap(),
                [("a".into(), a.clone()), ("b".into(), b.clone())].into(),
            )
            .unwrap();
            let fast = CreateIntent {
                placement: Some(Placement::FastSandbox),
                ..Default::default()
            };
            let requests = [
                CreateIntent {
                    needs: Some(Needs {
                        command_sessions: true,
                        ..Default::default()
                    }),
                    ..fast.clone()
                },
                CreateIntent {
                    needs: Some(Needs {
                        memory_suspend: true,
                        ..Default::default()
                    }),
                    ..fast.clone()
                },
                CreateIntent {
                    needs: Some(Needs {
                        profiles: [Profile::Fork].into(),
                        ..Default::default()
                    }),
                    ..fast.clone()
                },
                CreateIntent {
                    size: Some("unavailable".into()),
                    ..fast.clone()
                },
                CreateIntent {
                    image: Some("projects/fixture/global/images/base".into()),
                    ..fast.clone()
                },
                CreateIntent {
                    enrolled_host: Some("host-1".into()),
                    placement: Some(Placement::Enrolled),
                    ..Default::default()
                },
                CreateIntent {
                    egress_profile: Some("not-enforced".into()),
                    ..fast.clone()
                },
            ];
            for request in requests {
                assert!(matches!(
                    store
                        .create_placed("alice", &request, registry.catalog())
                        .await,
                    Err(oa_computers::store::CreateError::Admission(_))
                ));
            }
            assert!(
                store
                    .create_placed("bob", &fast, registry.catalog())
                    .await
                    .is_err()
            );
            let count: i64 = db
                .client()
                .await
                .unwrap()
                .query_one("SELECT count(*) FROM computers.computers", &[])
                .await
                .unwrap()
                .get(0);
            assert_eq!(count, 0);
            assert!(a.fake.calls().is_empty() && b.fake.calls().is_empty());
            let (entry, _) = store
                .create_placed("alice", &fast, registry.catalog())
                .await
                .unwrap();
            let entry = phase(&db, &store, &registry, &entry.id, "awake").await;
            let bound = registry.resolve(&entry).unwrap();
            let before = b.fake.calls().len();
            assert!(matches!(
                bound
                    .fork(entry.record.resource().unwrap(), "must-not-fork")
                    .await,
                Outcome::Failed { .. }
            ));
            assert!(matches!(
                bound
                    .snapshot(entry.record.resource().unwrap(), "must-not-snapshot")
                    .await,
                Outcome::Failed { .. }
            ));
            assert_eq!(b.fake.calls().len(), before);
            let (base, task) = server(store.clone(), &registry).await;
            assert_eq!(
                post(
                    &format!("{base}/v1/computers/{}/commands", entry.id),
                    "alice",
                    json!({"command":"true","idempotency_key":"unsupported"})
                )
                .await
                .0,
                422
            );
            let count: i64 = db
                .client()
                .await
                .unwrap()
                .query_one("SELECT count(*) FROM computers.commands", &[])
                .await
                .unwrap()
                .get(0);
            assert_eq!(count, 0);
            let changed = CreateIntent {
                idempotency_key: Some("same-key".into()),
                ..fast.clone()
            };
            let (original, _) = store
                .create_placed("alice", &changed, registry.catalog())
                .await
                .unwrap();
            let conflict = CreateIntent {
                needs: Some(Needs {
                    files: true,
                    ..Default::default()
                }),
                ..changed
            };
            assert!(matches!(
                store
                    .create_placed("alice", &conflict, registry.catalog())
                    .await,
                Err(oa_computers::store::CreateError::Conflict)
            ));
            for id in [&entry.id, &original.id] {
                store
                    .set_desired("alice", id, Desired::Deleted)
                    .await
                    .unwrap();
                phase(&db, &store, &registry, id, "deleted").await;
            }
            assert!(a.fake.live().is_empty() && b.fake.live().is_empty());
            task.abort();
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deployment_capacity_is_atomic_across_accounts_and_counts_pending_deletion() {
    let Some(db) = database().await else {
        return;
    };
    let store = Store::new(db.clone());
    let root = tempfile::tempdir().unwrap();
    let a = Simulated::new(root.path(), false);
    let mut target = a.target("a");
    target.capacity = Some(1);
    let catalog = Catalog::new("a", "p1", [target]).unwrap();
    let request = CreateIntent {
        idempotency_key: Some("capacity".into()),
        ..Default::default()
    };
    let (alice, bob) = tokio::join!(
        store.create_placed("alice", &request, &catalog),
        store.create_placed("bob", &request, &catalog)
    );
    assert_eq!(usize::from(alice.is_ok()) + usize::from(bob.is_ok()), 1);
    let ((entry, _), other) = match (alice, bob) {
        (
            Ok(row),
            Err(oa_computers::store::CreateError::Admission(
                oa_computers::registry::Refused::Capacity,
            )),
        ) => (row, "bob"),
        (
            Err(oa_computers::store::CreateError::Admission(
                oa_computers::registry::Refused::Capacity,
            )),
            Ok(row),
        ) => (row, "alice"),
        pair => panic!("unexpected capacity results: {pair:?}"),
    };
    store
        .set_desired(&entry.owner, &entry.id, Desired::Deleted)
        .await
        .unwrap();
    assert!(matches!(
        store.create_placed(other, &request, &catalog).await,
        Err(oa_computers::store::CreateError::Admission(
            oa_computers::registry::Refused::Capacity
        ))
    ));
    assert!(a.fake.calls().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_saved_memory_requirement_cannot_become_a_disk_only_stop_after_restart() {
    let Some(db) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async {
            let store = Store::new(db.clone());
            let root = tempfile::tempdir().unwrap();
            let mut b = Simulated::new(root.path(), true);
            b.caps.memory_suspend = true;
            let original = Registry::new(
                Catalog::new("b", "p1", [b.target("b")]).unwrap(),
                [("b".into(), b.clone())].into(),
            )
            .unwrap();
            let request = CreateIntent {
                placement: Some(Placement::FastSandbox),
                needs: Some(Needs {
                    memory_suspend: true,
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (entry, _) = store
                .create_placed("alice", &request, original.catalog())
                .await
                .unwrap();
            let entry = phase(&db, &store, &original, &entry.id, "awake").await;
            let mut changed = b.clone();
            changed.caps.memory_suspend = false;
            let restarted = Registry::new(
                Catalog::new("b", "p2", [changed.target("b")]).unwrap(),
                [("b".into(), changed)].into(),
            )
            .unwrap();
            let bound = restarted.resolve(&entry).unwrap();
            let resource = entry.record.resource().unwrap();
            let before = b.fake.calls().len();
            assert!(matches!(
                bound.suspend(resource).await,
                Outcome::Failed { .. }
            ));
            assert!(matches!(
                bound.resume(resource).await,
                Outcome::Failed { .. }
            ));
            assert!(matches!(
                bound
                    .create(&CreateRequest {
                        operation: "pending".into(),
                        size: Some(entry.size.clone()),
                        image: entry.image.clone(),
                        ttl_seconds: None,
                        labels: Default::default()
                    })
                    .await,
                Outcome::Failed { .. }
            ));
            assert_eq!(b.fake.calls().len(), before);
            assert_eq!(b.fake.live().len(), 1);
            store
                .set_desired("alice", &entry.id, Desired::Deleted)
                .await
                .unwrap();
            phase(&db, &store, &restarted, &entry.id, "deleted").await;
            assert!(b.fake.live().is_empty());
        })
        .await;
}
