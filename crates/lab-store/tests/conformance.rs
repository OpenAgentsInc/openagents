use axum::{
    Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use lab_kit::{echo::Echo, *};
use lab_store::{
    backend::{Backend, Bearer, CloudBackend, FsBackend, GcsObjects},
    *,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

fn initial(text: &str) -> CheckedDraft {
    CheckedDraft::initial::<Echo>("echo.v1", json!({"text":text}), "Created", &Value::Null).unwrap()
}
fn edit(version: &Version, text: &str) -> CheckedDraft {
    CheckedDraft::change::<Echo>(
        version,
        &Change::Intent {
            intent: json!({"intent":"set","text":text}),
        },
        "Edited",
        &Value::Null,
    )
    .unwrap()
}
fn space(account: &str) -> SpaceRef {
    SpaceRef::Personal {
        account: account.into(),
    }
}

async fn conformance(backend: Arc<dyn Backend>) {
    let unique = format!(
        "test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let store = Store::new(backend.clone(), &space(&unique)).unwrap();
    let other = Store::new(backend.clone(), &space(&format!("{unique}-other"))).unwrap();
    let project = Store::new(
        backend.clone(),
        &SpaceRef::Project {
            project: unique.clone(),
        },
    )
    .unwrap();
    let v1 = store
        .create("doc", "Example", "test", initial("one"), 1)
        .await
        .unwrap();
    assert!(matches!(other.document("doc").await, Err(Error::NotFound)));
    assert!(matches!(
        project.document("doc").await,
        Err(Error::NotFound)
    ));
    assert_eq!(
        store
            .documents(Some("echo"), 0, 10)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(store.documents(None, 0, 101).await.is_err());
    let mut forged = v1.clone();
    forged.content = json!({"text":"not the saved base"});
    assert!(matches!(
        store.save("doc", "test", edit(&forged, "changed"), 2).await,
        Err(Error::Conflict)
    ));
    assert_eq!(store.document("doc").await.unwrap().latest, 1);
    let a = store.clone();
    let b = Store::new(backend.clone(), &space(&unique)).unwrap();
    let draft_a = edit(&v1, "winner a");
    let draft_b = edit(&v1, "winner b");
    let (a, b) = tokio::join!(
        a.save("doc", "a", draft_a, 2),
        b.save("doc", "b", draft_b, 2)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let loser = if a.is_err() {
        a.unwrap_err()
    } else {
        b.unwrap_err()
    };
    assert!(matches!(loser, Error::Conflict));
    assert_eq!(store.document("doc").await.unwrap().latest, 2);
    assert_eq!(store.version("doc", 1).await.unwrap(), v1);
    let p = store.versions("doc", 0, 1).await.unwrap();
    assert_eq!(p.next, Some(1));
    assert_eq!(
        store.versions("doc", 1, 1).await.unwrap().items[0].number,
        2
    );
    let artifact = store
        .put_artifact(
            ArtifactInput {
                version: v1.reference(),
                role: ArtifactRole::Export,
                media_type: "text/plain".into(),
                class: AssetClass::NoTraining,
                no_training: false,
            },
            b"abcdef".to_vec(),
            20,
            3,
        )
        .await
        .unwrap();
    assert!(artifact.no_training);
    assert_eq!(
        store
            .artifact_bytes(&artifact.id, Some(1..4))
            .await
            .unwrap(),
        b"bcd"
    );
    assert!(
        store
            .artifact_bytes(&artifact.id, Some(3..7))
            .await
            .is_err()
    );
    assert!(matches!(
        other.artifact(&artifact.id).await,
        Err(Error::NotFound)
    ));
    assert!(other.artifact_bytes(&artifact.id, None).await.is_err());
    assert!(store.artifact_url(&artifact.id, 901).await.is_err());
    let spec = Echo::jobs_for(&v1, &Want::Card).remove(0);
    let manifest = Echo::manifest();
    let (j1, j2) = tokio::join!(
        store.create_job(spec.clone(), &manifest, 4),
        store.create_job(spec.clone(), &manifest, 4)
    );
    let job = j1.unwrap();
    assert_eq!(job, j2.unwrap());
    assert_eq!(store.jobs("doc", 0, 10).await.unwrap().items.len(), 1);
    assert!(matches!(other.job(&job.id).await, Err(Error::NotFound)));
    let lease = store.claim(&job.id, "worker-a", 5, 10).await.unwrap();
    assert!(store.claim(&job.id, "worker-b", 6, 10).await.is_err());
    assert_eq!(store.expire_leases(15).await.unwrap(), 1);
    assert_eq!(store.job(&job.id).await.unwrap().status, JobStatus::Queued);
    let next = store.claim(&job.id, "worker-b", 16, 10).await.unwrap();
    assert!(next.attempt > lease.attempt);
    assert!(matches!(
        store.heartbeat(&lease, 17, 20).await,
        Err(Error::LeaseLost)
    ));
    let next = store.start(&next, 17).await.unwrap();
    let next = store.heartbeat(&next, 18, 20).await.unwrap();
    assert_eq!(next.expires_unix_ms, 38);
    assert_eq!(store.expire_leases(39).await.unwrap(), 1);
    assert_eq!(
        store.job(&job.id).await.unwrap().status,
        JobStatus::Uncertain
    );
    assert!(store.claim(&job.id, "worker-c", 40, 10).await.is_err());
    assert!(matches!(
        store.finish(&next, JobStatus::Done, None, vec![], 40).await,
        Err(Error::LeaseLost)
    ));
    assert_eq!(
        store
            .create_job(spec, &Echo::manifest(), 41)
            .await
            .unwrap()
            .status,
        JobStatus::Uncertain
    );
    let mut second = Echo::jobs_for(&v1, &Want::Card).remove(0);
    second.params = json!({"attempt":"second"});
    let j = store
        .create_job(second, &Echo::manifest(), 42)
        .await
        .unwrap();
    let l = store.claim(&j.id, "worker", 43, 100).await.unwrap();
    let l = store.start(&l, 44).await.unwrap();
    assert!(
        store
            .finish(&l, JobStatus::Done, None, vec!["missing".into()], 45)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .finish(&l, JobStatus::Done, None, vec![artifact.id.clone()], 46)
            .await
            .unwrap()
            .status,
        JobStatus::Done
    );
    assert_eq!(
        store
            .finish(&l, JobStatus::Done, None, vec![artifact.id.clone()], 1000)
            .await
            .unwrap()
            .status,
        JobStatus::Done
    );
    let mut wrong = l.clone();
    wrong.worker = "another-worker".into();
    assert!(matches!(
        store
            .finish(
                &wrong,
                JobStatus::Done,
                None,
                vec![artifact.id.clone()],
                1000
            )
            .await,
        Err(Error::LeaseLost)
    ));
    let note = Note {
        subject: v1.reference(),
        locator: Locator::Frame { frame: 3 },
        author: "test".into(),
        text: "Change this".into(),
        created_unix_ms: 47,
    };
    store.add_note(note.clone()).await.unwrap();
    assert_eq!(store.notes("doc", 0, 10).await.unwrap().items, vec![note]);
    let events = store.events("doc", 0, 100).await.unwrap().items;
    assert!(events.len() >= 10);
    assert!(events.windows(2).all(|w| w[0].cursor < w[1].cursor));
    let first = store.events("doc", 0, 2).await.unwrap();
    assert_eq!(first.next, Some(first.items[1].cursor));
    assert_eq!(
        store
            .events("doc", first.next.unwrap(), 100)
            .await
            .unwrap()
            .items[0],
        events[2]
    );
    let reopen = Store::new(backend, &space(&unique)).unwrap();
    assert_eq!(reopen.document("doc").await.unwrap().latest, 2);
    store.delete("doc", 48).await.unwrap();
    assert!(matches!(
        reopen.version("doc", 1).await,
        Err(Error::NotFound)
    ));
    assert!(store.artifact_bytes(&artifact.id, None).await.is_err());
    assert!(
        store
            .create("doc", "Reused", "test", initial("new"), 49)
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn filesystem_conformance() {
    let root = tempfile::tempdir().unwrap();
    conformance(Arc::new(FsBackend::open(root.path()).unwrap())).await;
}

struct TestBearer;
#[async_trait::async_trait]
impl Bearer for TestBearer {
    async fn token(&self) -> lab_store::Result<String> {
        Ok("isolated-storage-test".into())
    }
}
#[derive(Clone, Default)]
struct FakeGcs(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);
async fn upload(
    State(s): State<FakeGcs>,
    Query(q): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    assert_eq!(
        headers.get("authorization").unwrap(),
        "Bearer isolated-storage-test"
    );
    assert_eq!(q["ifGenerationMatch"], "0");
    assert_eq!(q["uploadType"], "media");
    let mut map = s.0.lock().unwrap();
    if map.contains_key(&q["name"]) {
        return StatusCode::PRECONDITION_FAILED;
    }
    map.insert(q["name"].clone(), bytes.to_vec());
    StatusCode::OK
}
async fn download(
    State(s): State<FakeGcs>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> axum::response::Response {
    assert_eq!(
        headers.get("authorization").unwrap(),
        "Bearer isolated-storage-test"
    );
    let map = s.0.lock().unwrap();
    let Some(bytes) = map.get(&key) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(range) = headers.get("range") {
        let r = range.to_str().unwrap().strip_prefix("bytes=").unwrap();
        let (a, b) = r.split_once('-').unwrap();
        let a: usize = a.parse().unwrap();
        let b: usize = b.parse().unwrap();
        if a > b || b >= bytes.len() {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        }
        return (
            StatusCode::PARTIAL_CONTENT,
            [("Content-Range", format!("bytes {a}-{b}/{}", bytes.len()))],
            bytes[a..=b].to_vec(),
        )
            .into_response();
    }
    bytes.clone().into_response()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_and_gcs_conformance() {
    let Ok(url) = std::env::var("LAB_STORE_TEST_DATABASE") else {
        eprintln!(
            "Set LAB_STORE_TEST_DATABASE to an isolated loopback PostgreSQL database to run cloud conformance."
        );
        return;
    };
    let parsed = reqwest::Url::parse(&url).unwrap();
    assert!(
        matches!(parsed.host_str(), Some("127.0.0.1" | "localhost")),
        "Use a disposable local database"
    );
    let fake = FakeGcs::default();
    let router = Router::new()
        .route("/upload/storage/v1/b/test-bucket/o", post(upload))
        .route("/download/storage/v1/b/test-bucket/o/{key}", get(download))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let objects = GcsObjects::emulator(
        &format!("http://{addr}/"),
        "test-bucket",
        Arc::new(TestBearer),
    )
    .unwrap();
    let pool = sqlx_postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    conformance(Arc::new(
        CloudBackend::open(pool.clone(), Arc::new(objects))
            .await
            .unwrap(),
    ))
    .await;
    assert!(!fake.0.lock().unwrap().is_empty());
    pool.close().await;
    server.abort();
}

#[tokio::test]
async fn filesystem_rejects_git_roots_and_symlinks() {
    assert!(FsBackend::open("relative").is_err());
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    assert!(FsBackend::open(root.path().join("inside")).is_err());
    #[cfg(unix)]
    {
        let clean = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), clean.path().join("shortcut")).unwrap();
        assert!(FsBackend::open(clean.path().join("shortcut")).is_err());
        let backend = FsBackend::open(clean.path()).unwrap();
        let space = "a".repeat(64);
        std::os::unix::fs::symlink(outside.path(), clean.path().join(&space)).unwrap();
        assert!(backend.load(&space).await.is_err());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(clean.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
