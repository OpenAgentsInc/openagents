//! Real outbound HTTP and a real bounded child; all account/host state is under
//! this fixture's temporary directory on the disposable qualification machine.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use lab_jobs::{
    Capabilities, Placement,
    authority::{Authority, Enrolled},
    coordinator::Coordinator,
    protocol,
    worker::{Config, Handler, Worker},
};
use lab_kit::{AssetClass, AssetEntry, JobStatus, LabKind, SpaceRef, Want, echo::Echo};
use lab_store::{CheckedDraft, LabStore, Store, backend::FsBackend};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
struct Server {
    coordinator: Coordinator,
    store: Store,
    kinds: BTreeMap<String, lab_kit::KindManifest>,
    captured: Mutex<Vec<Vec<u8>>>,
    allowed: AtomicBool,
    lose_finish: AtomicBool,
    starts: AtomicUsize,
}
type S = State<Arc<Server>>;
async fn auth(s: &Server, h: &HeaderMap) -> Result<lab_jobs::authority::Worker, StatusCode> {
    if !s.allowed.load(Ordering::SeqCst) {
        return Err(StatusCode::FORBIDDEN);
    }
    s.coordinator
        .authority
        .authenticate(
            h.get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .unwrap_or(""),
            now(),
        )
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)
}
fn record(s: &Server, v: &impl serde::Serialize) {
    s.captured
        .lock()
        .unwrap()
        .push(serde_json::to_vec(v).unwrap());
}
async fn poll(
    State(s): S,
    h: HeaderMap,
    Json(r): Json<protocol::Poll>,
) -> Result<Json<protocol::Available>, StatusCode> {
    record(&s, &r);
    let w = auth(&s, &h).await?;
    let work = s
        .coordinator
        .poll(&w, &s.store, &s.kinds, now())
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    if let Some(work) = &work {
        record(&s, work);
    }
    Ok(Json(protocol::Available { work }))
}
async fn start(
    State(s): S,
    h: HeaderMap,
    Json(r): Json<protocol::Attempt>,
) -> Result<Json<protocol::Work>, StatusCode> {
    record(&s, &r);
    let w = auth(&s, &h).await?;
    s.starts.fetch_add(1, Ordering::SeqCst);
    Ok(Json(
        s.coordinator
            .start(&w, &s.store, &s.kinds, &r, now())
            .await
            .map_err(|_| StatusCode::CONFLICT)?,
    ))
}
async fn heartbeat(
    State(s): S,
    h: HeaderMap,
    Json(r): Json<protocol::Attempt>,
) -> Result<Json<protocol::Work>, StatusCode> {
    record(&s, &r);
    let w = auth(&s, &h).await?;
    Ok(Json(
        s.coordinator
            .heartbeat(&w, &s.store, &s.kinds, &r, now())
            .await
            .map_err(|_| StatusCode::CONFLICT)?,
    ))
}
async fn finish(
    State(s): S,
    h: HeaderMap,
    Json(r): Json<protocol::Finish>,
) -> Result<Json<lab_kit::JobState>, StatusCode> {
    record(&s, &r);
    let w = auth(&s, &h).await?;
    let result = s
        .coordinator
        .finish(&w, &s.store, &s.kinds, &r, now())
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    if s.lose_finish.swap(false, Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    Ok(Json(result))
}

fn private_json(path: &Path, value: &impl serde::Serialize) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
struct PrivateEcho;
impl LabKind for PrivateEcho {
    const KIND: &'static str = "echo";
    fn manifest() -> lab_kit::KindManifest {
        Echo::manifest()
    }
    fn check(c: &Value, m: &Value) -> Result<lab_kit::Report, lab_kit::Refusal> {
        Echo::check(c, m)
    }
    fn apply(
        base: &lab_kit::Version,
        change: &lab_kit::Change,
        summary: &str,
        m: &Value,
    ) -> Result<lab_kit::Draft, lab_kit::Refusal> {
        let mut draft = Echo::apply(base, change, summary, m)?;
        draft.assets = serde_json::from_value(m.clone())
            .map_err(|_| lab_kit::Refusal::new("fixture", "Check fixture assets."))?;
        Ok(draft)
    }
    fn jobs_for(v: &lab_kit::Version, w: &Want) -> Vec<lab_kit::JobSpec> {
        Echo::jobs_for(v, w)
    }
    fn card(
        v: &lab_kit::Version,
        j: &[lab_kit::JobState],
        a: &[lab_kit::Artifact],
    ) -> lab_kit::Card {
        Echo::card(v, j, a)
    }
}
struct Fixture {
    tmp: tempfile::TempDir,
    s: Arc<Server>,
    credential: Enrolled,
    job: String,
    origin: String,
    task: tokio::task::JoinHandle<()>,
    private_file: Option<(String, std::path::PathBuf)>,
}
impl Fixture {
    async fn new(placement: Placement, private: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::open(tmp.path().join("store")).unwrap());
        let space = SpaceRef::Personal {
            account: "fixture-account".into(),
        };
        let store = Store::new(backend.clone(), &space).unwrap();
        let private_file = if private {
            let path = tmp.path().join("private-source");
            fs::write(&path, b"PRIVATE-INPUT-DO-NOT-UPLOAD-11878").unwrap();
            Some((
                format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
                path,
            ))
        } else {
            None
        };
        let draft =
            CheckedDraft::initial::<Echo>("echo.v1", json!({"text":"hello"}), "Echo", &Value::Null)
                .unwrap();
        let mut version = store
            .create("echo-job", "Echo", "fixture", draft, now())
            .await
            .unwrap();
        if let Some((hash, _)) = &private_file {
            let change = lab_kit::Change::Intent {
                intent: json!({"intent":"set","text":"hello"}),
            };
            let assets = json!([AssetEntry {
                location: "local/input".into(),
                sha256: hash.clone(),
                class: AssetClass::PrivateLocal
            }]);
            let draft =
                CheckedDraft::change::<PrivateEcho>(&version, &change, "Private input", &assets)
                    .unwrap();
            version = store
                .save("echo-job", "fixture", draft, now())
                .await
                .unwrap();
        }
        let manifest = Echo::manifest();
        let mut spec = Echo::jobs_for(&version, &Want::Card).remove(0);
        spec.requirements.private_local = private;
        let job = store.create_job(spec, &manifest, now()).await.unwrap().id;
        let authority = Authority::new(backend);
        let credential = authority
            .enroll(
                "fixture-account",
                space,
                Capabilities {
                    worker: "fixture".into(),
                    placement,
                    cpu_millis: 1000,
                    memory_bytes: 256 * 1024 * 1024,
                    gpu: false,
                    tools: Default::default(),
                    jobs: ["echo/shout".into()].into(),
                    local_assets: private_file
                        .as_ref()
                        .map(|(h, _)| [h.clone()].into())
                        .unwrap_or_default(),
                    max_seconds: 600,
                },
                now(),
                60000,
            )
            .await
            .unwrap();
        let s = Arc::new(Server {
            coordinator: Coordinator { authority },
            store,
            kinds: [("echo".into(), manifest)].into(),
            captured: Mutex::new(vec![]),
            allowed: AtomicBool::new(true),
            lose_finish: AtomicBool::new(false),
            starts: AtomicUsize::new(0),
        });
        let app = Router::new()
            .route("/v1/labs/workers/poll", post(poll))
            .route("/v1/labs/workers/start", post(start))
            .route("/v1/labs/workers/heartbeat", post(heartbeat))
            .route("/v1/labs/workers/finish", post(finish))
            .with_state(s.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            tmp,
            s,
            credential,
            job,
            origin,
            task,
            private_file,
        }
    }
    fn config(&self) -> std::path::PathBuf {
        let credential = self.tmp.path().join("credential.json");
        private_json(&credential, &self.credential);
        let config = Config {
            origin: self.origin.clone(),
            credential,
            state: self.tmp.path().join("worker"),
            handlers: [(
                "echo/shout".into(),
                Handler {
                    executable: Path::new(env!("CARGO_BIN_EXE_labs-worker"))
                        .canonicalize()
                        .unwrap(),
                    arguments: vec!["--echo-handler".into()],
                },
            )]
            .into(),
            local_assets: self.private_file.clone().into_iter().collect(),
            allow_loopback_http: true,
        };
        let path = self.tmp.path().join("config.json");
        private_json(&path, &config);
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
#[tokio::test]
async fn real_child_on_each_placement_and_private_input_stays_local() {
    for (placement, private) in [
        (Placement::Server, false),
        (Placement::Cloud, false),
        (Placement::Enrolled, true),
    ] {
        let f = Fixture::new(placement, private).await;
        let config = f.config();
        let worker = Worker::open(&config).unwrap();
        assert!(Worker::open(&config).is_err(), "exclusive process lock");
        assert!(worker.run_once().await.unwrap());
        let job = f.s.store.job(&f.job).await.unwrap();
        assert_eq!(job.status, JobStatus::Done);
        assert_eq!(job.artifacts.len(), 1);
        let artifact = f.s.store.artifact(&job.artifacts[0]).await.unwrap();
        assert_eq!(artifact.no_training, private);
        assert_eq!(
            artifact.class,
            if private {
                AssetClass::NoTraining
            } else {
                AssetClass::Public
            }
        );
        let output = f.s.store.artifact_bytes(&artifact.id, None).await.unwrap();
        let output: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(output["text"], "HELLO");
        let usage =
            f.s.coordinator
                .authority
                .usage(&f.credential.worker.space, &f.job)
                .await
                .unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].placement, placement);
        assert_eq!(usage[0].charge, None);
        let wire = f.s.captured.lock().unwrap().concat();
        use base64::Engine;
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(b"PRIVATE-INPUT-DO-NOT-UPLOAD-11878");
        assert!(!String::from_utf8_lossy(&wire).contains(&encoded));
        assert!(
            !wire
                .windows(b"PRIVATE-INPUT-DO-NOT-UPLOAD-11878".len())
                .any(|w| w == b"PRIVATE-INPUT-DO-NOT-UPLOAD-11878")
        );
        if let Some((hash, path)) = &f.private_file {
            assert_eq!(
                fs::read(path).unwrap(),
                b"PRIVATE-INPUT-DO-NOT-UPLOAD-11878"
            );
            assert_eq!(output["input_checksums"][0], *hash);
            assert!(!String::from_utf8_lossy(&wire).contains(path.to_str().unwrap()));
        }
    }
}

#[tokio::test]
async fn lost_finish_reply_recovers_saved_output_without_another_child() {
    let f = Fixture::new(Placement::Server, false).await;
    let config = f.config();
    f.s.lose_finish.store(true, Ordering::SeqCst);
    {
        let worker = Worker::open(&config).unwrap();
        assert!(worker.run_once().await.is_err());
    }
    assert_eq!(f.s.store.job(&f.job).await.unwrap().status, JobStatus::Done);
    let worker = Worker::open(&config).unwrap();
    assert!(worker.run_once().await.unwrap());
    assert_eq!(f.s.starts.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.s.coordinator
            .authority
            .usage(&f.credential.worker.space, &f.job)
            .await
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn restart_after_execution_marker_never_replays_the_effect() {
    let f = Fixture::new(Placement::Enrolled, false).await;
    let config = f.config();
    let worker = Worker::open(&config).unwrap();
    let w = &f.credential.worker;
    let work =
        f.s.coordinator
            .poll(w, &f.s.store, &f.s.kinds, now())
            .await
            .unwrap()
            .unwrap();
    let attempt = protocol::Attempt {
        job: f.job.clone(),
        attempt: work.assignment.lease.attempt,
    };
    let work =
        f.s.coordinator
            .start(w, &f.s.store, &f.s.kinds, &attempt, now())
            .await
            .unwrap();
    private_json(
        &f.tmp.path().join("worker/job.json"),
        &json!({"work":work,"execution_marked":true,"result":null}),
    );
    assert!(worker.run_once().await.unwrap());
    assert_eq!(
        f.s.store.job(&f.job).await.unwrap().status,
        JobStatus::Uncertain
    );
    assert_eq!(f.s.starts.load(Ordering::SeqCst), 0);
    assert!(f.s.store.job(&f.job).await.unwrap().artifacts.is_empty());
}
#[tokio::test]
async fn permission_loss_stops_a_running_child_and_never_publishes_its_output() {
    let f = Fixture::new(Placement::Enrolled, false).await;
    let config = f.config();
    let mut settings: Config = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    settings.handlers.insert(
        "echo/shout".into(),
        Handler {
            executable: Path::new("/bin/sleep").canonicalize().unwrap(),
            arguments: vec!["30".into()],
        },
    );
    private_json(&config, &settings);
    let worker = Worker::open(&config).unwrap();
    let started = std::time::Instant::now();
    let lose_access = async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        f.s.allowed.store(false, Ordering::SeqCst);
    };
    let (result, ()) = tokio::join!(worker.run_once(), lose_access);
    assert!(result.is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    f.s.coordinator
        .observe_losses(&f.credential.worker.space, &f.s.store, now() + 31_000)
        .await
        .unwrap();
    assert_eq!(
        f.s.store.job(&f.job).await.unwrap().status,
        JobStatus::Uncertain
    );
    let usage =
        f.s.coordinator
            .authority
            .usage(&f.credential.worker.space, &f.job)
            .await
            .unwrap();
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].observed_wall_ms, None);
    assert_eq!(usage[0].charge, None);
}
