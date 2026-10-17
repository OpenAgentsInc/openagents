use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use lab_api::{Access, Action, Api, ApiError, Authorizer, Gate, KindAdapter, Pin, Registry};
use lab_jobs::{
    authority::{Authority, Enrolled},
    protocol,
};
use lab_kit::{
    Artifact, ArtifactRole, AssetClass, AssetEntry, Card, Change, Draft, JobSpec, JobState,
    KindManifest, LabKind, Limits, Measurements, Refusal, Report, SpaceRef, Version, View, Want,
};
use lab_store::{ArtifactInput, CheckedDraft, LabStore, Store, backend::FsBackend};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tower::ServiceExt;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
struct Auth {
    credentials: Authority,
    allowed: AtomicBool,
}
fn bearer(h: &HeaderMap) -> &str {
    h.get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
}
#[async_trait::async_trait]
impl Authorizer for Auth {
    async fn authorize(&self, h: &HeaderMap, space: &str, _: Action) -> Result<Access, ApiError> {
        let owner = bearer(h);
        if !["alice", "bob"].contains(&owner) || space != "me" {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "signed_out",
                "Sign in to continue.",
            ));
        }
        Ok(Access {
            account: owner.into(),
            space: SpaceRef::Personal {
                account: owner.into(),
            },
            experimental: true,
            cad: false,
        })
    }
    async fn enroll_worker(
        &self,
        h: &HeaderMap,
        r: &protocol::Enroll,
    ) -> Result<Enrolled, ApiError> {
        let a = self.authorize(h, &r.space, Action::Edit).await?;
        Ok(self
            .credentials
            .enroll(
                &a.account,
                a.space,
                r.capabilities.clone(),
                now(),
                r.lifetime_ms,
            )
            .await?)
    }
    async fn worker_access(&self, h: &HeaderMap) -> Result<protocol::Access, ApiError> {
        if !self.allowed.load(Ordering::SeqCst) {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "project_role",
                "This worker can no longer use this space.",
            ));
        }
        Ok(protocol::Access {
            worker: self.credentials.authenticate(bearer(h), now()).await?,
            experimental: true,
            cad: false,
        })
    }
    async fn revoke_worker(&self, h: &HeaderMap, id: &str) -> Result<(), ApiError> {
        let account = if bearer(h).starts_with("labw_") {
            let worker = self.credentials.authenticate(bearer(h), now()).await?;
            if worker.id != id {
                return Err(ApiError::missing());
            }
            worker.account
        } else {
            self.authorize(h, "me", Action::Edit).await?.account
        };
        self.credentials.revoke(&account, id).await?;
        Ok(())
    }
}

/// A kind whose content names other versions by content digest in `uses`.
struct Linked;
impl LabKind for Linked {
    const KIND: &'static str = "linked";
    fn manifest() -> KindManifest {
        KindManifest {
            kind: Self::KIND.into(),
            name: "Linked".into(),
            schemas: vec!["linked.v1".into()],
            tool_prefix: "linked_".into(),
            doc_arg: "document".into(),
            tools: vec![],
            job_types: vec!["run".into()],
            job_requirements: Default::default(),
            views: vec![View::Report],
            limits: Limits::default(),
        }
    }
    fn check(_: &Value, _: &Measurements) -> Result<Report, Refusal> {
        Ok(Report::default())
    }
    fn apply(_: &Version, c: &Change, s: &str, _: &Measurements) -> Result<Draft, Refusal> {
        let Change::Intent { intent } = c else {
            return Err(Refusal::new("intent", "Use an intent."));
        };
        Ok(Draft {
            content: intent["content"].clone(),
            summary: s.into(),
            report: Report::default(),
            assets: serde_json::from_value(intent["assets"].clone()).unwrap_or_default(),
        })
    }
    fn jobs_for(v: &Version, _: &Want) -> Vec<JobSpec> {
        vec![JobSpec {
            kind: Self::KIND.into(),
            job_type: "run".into(),
            version: v.reference(),
            params: Value::Null,
            requirements: Default::default(),
        }]
    }
    fn card(v: &Version, _: &[JobState], _: &[Artifact]) -> Card {
        lab_kit::echo::Echo::card(v, &[], &[])
    }
}
fn pins(content: &Value) -> Result<Vec<Pin>, Refusal> {
    Ok(content["uses"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|u| Pin {
            document: u["document"].as_str().unwrap_or_default().into(),
            content_sha256: u["content_sha256"].as_str().unwrap_or_default().into(),
        })
        .collect())
}

struct Fixture {
    app: Router,
    auth: Arc<Auth>,
    store: Store,
    _tmp: tempfile::TempDir,
    job: String,
    used: Artifact,
    outside: Artifact,
}
fn bytes(n: u8) -> Vec<u8> {
    vec![n; 64]
}
async fn file(store: &Store, v: &Version, class: AssetClass, n: u8) -> Artifact {
    store
        .put_artifact(
            ArtifactInput {
                version: v.reference(),
                role: ArtifactRole::Source,
                media_type: "application/octet-stream".into(),
                class,
                no_training: class != AssetClass::Public,
            },
            bytes(n),
            1024,
            now(),
        )
        .await
        .unwrap()
}
async fn create(store: &Store, id: &str, content: Value) -> Version {
    store
        .create(
            id,
            id,
            "alice",
            CheckedDraft::initial::<Linked>("linked.v1", content, "First", &Value::Null).unwrap(),
            now(),
        )
        .await
        .unwrap()
}
impl Fixture {
    /// A job on `root`, which pins `robot`; `other` is saved but not referenced.
    async fn new(private_reference: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::open(tmp.path().join("labs")).unwrap());
        let credentials = Authority::new(Arc::new(
            FsBackend::open(tmp.path().join("accounts")).unwrap(),
        ));
        let auth = Arc::new(Auth {
            credentials,
            allowed: AtomicBool::new(true),
        });
        let mut registry = Registry::default();
        registry
            .register(
                Arc::new(
                    KindAdapter::<Linked>::new(|_, _| Err(ApiError::missing())).with_pins(pins),
                ),
                Gate::Experimental,
            )
            .unwrap();
        let store = Store::new(
            backend.clone(),
            &SpaceRef::Personal {
                account: "alice".into(),
            },
        )
        .unwrap();
        let mut robot = create(&store, "robot", json!({"name": "arm"})).await;
        if private_reference {
            let draft = CheckedDraft::change::<Linked>(
                &robot,
                &Change::Intent {
                    intent: json!({"content": {"name": "arm", "mesh": 2}, "assets": [AssetEntry {
                        location: "~/meshes/arm.glb".into(),
                        sha256: "ab".repeat(32),
                        class: AssetClass::PrivateLocal,
                    }]}),
                },
                "Private mesh",
                &Value::Null,
            )
            .unwrap();
            robot = store.save("robot", "alice", draft, now()).await.unwrap();
        }
        let used = file(&store, &robot, AssetClass::Public, 1).await;
        let other = create(&store, "other", json!({"name": "unrelated"})).await;
        let outside = file(&store, &other, AssetClass::Public, 3).await;
        let root = create(
            &store,
            "root",
            json!({"uses": [{"document": "robot", "content_sha256": robot.content_sha256}]}),
        )
        .await;
        let job = store
            .create_job(
                Linked::jobs_for(&root, &Want::Card).remove(0),
                &Linked::manifest(),
                now(),
            )
            .await
            .unwrap()
            .id;
        Self {
            app: Arc::new(Api {
                backend,
                authorizer: auth.clone(),
                registry,
            })
            .router(),
            auth,
            store,
            _tmp: tmp,
            job,
            used,
            outside,
        }
    }
    async fn call(&self, token: &str, path: &str, body: Value) -> (StatusCode, Vec<u8>) {
        let r = Request::builder()
            .method("POST")
            .uri(path)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let r = self.app.clone().oneshot(r).await.unwrap();
        let status = r.status();
        assert_eq!(
            r.headers().get("Cache-Control").unwrap(),
            "private, no-store"
        );
        (
            status,
            r.into_body().collect().await.unwrap().to_bytes().to_vec(),
        )
    }
    async fn json(&self, token: &str, path: &str, body: Value) -> (StatusCode, Value) {
        let (s, b) = self.call(token, path, body).await;
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }
    /// Enrolls a worker at `placement` and starts the job; returns the key and attempt.
    async fn started(&self, placement: &str) -> (String, String, Value) {
        let (s, b) = self
            .json("alice", "/v1/labs/workers", json!({"space":"me","lifetime_ms":60000,"capabilities":{
                "worker":"fixture","placement":placement,"cpu_millis":1000,"memory_bytes":134217728,
                "gpu":false,"tools":[],"jobs":["linked/run"],"local_assets":[],"max_seconds":600,
            }}))
            .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let token = b["token"].as_str().unwrap().to_owned();
        let id = b["worker"]["id"].as_str().unwrap().to_owned();
        let (_, work) = self.json(&token, "/v1/labs/workers/poll", json!({})).await;
        assert_eq!(work["work"]["assignment"]["lease"]["job"]["id"], self.job);
        let attempt =
            json!({"job": self.job, "attempt": work["work"]["assignment"]["lease"]["attempt"]});
        assert_eq!(
            self.json(&token, "/v1/labs/workers/inputs", attempt.clone())
                .await
                .0,
            StatusCode::CONFLICT,
            "a lease that hasn't started reads nothing"
        );
        assert_eq!(
            self.json(&token, "/v1/labs/workers/start", attempt.clone())
                .await
                .0,
            StatusCode::OK
        );
        (token, id, attempt)
    }
    fn read(&self, attempt: &Value, artifact: &str) -> Value {
        json!({"job": attempt["job"], "attempt": attempt["attempt"], "artifact": artifact})
    }
}

#[tokio::test]
async fn a_started_job_reads_its_referenced_versions_and_their_public_files_only() {
    let f = Fixture::new(false).await;
    let (token, _, attempt) = f.started("cloud").await;
    let (s, inputs) = f
        .json(&token, "/v1/labs/workers/inputs", attempt.clone())
        .await;
    assert_eq!(s, StatusCode::OK, "{inputs}");
    let documents: Vec<_> = inputs["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["document"].as_str().unwrap())
        .collect();
    assert_eq!(documents, ["root", "robot"]);
    let files: Vec<_> = inputs["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(files, [f.used.id.as_str()]);

    let (s, body) = f
        .call(
            &token,
            "/v1/labs/workers/inputs/file",
            f.read(&attempt, &f.used.id),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, bytes(1));
    for outside in [&f.outside.id, &"missing".to_owned()] {
        assert_eq!(
            f.call(
                &token,
                "/v1/labs/workers/inputs/file",
                f.read(&attempt, outside)
            )
            .await
            .0,
            StatusCode::NOT_FOUND,
            "a file outside the referenced set is refused"
        );
    }
    let mut wrong = f.read(&attempt, &f.used.id);
    wrong["job"] = json!("another-job");
    assert_ne!(
        f.call(&token, "/v1/labs/workers/inputs/file", wrong)
            .await
            .0,
        StatusCode::OK
    );
    // An account bearer isn't a worker key.
    assert_eq!(
        f.call("alice", "/v1/labs/workers/inputs", attempt.clone())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_expired_lease_refuses_reads() {
    let f = Fixture::new(false).await;
    let (token, _, attempt) = f.started("cloud").await;
    f.store.expire_leases(now() + 120_000).await.unwrap();
    assert_ne!(
        f.json(&token, "/v1/labs/workers/inputs", attempt.clone())
            .await
            .0,
        StatusCode::OK
    );
    assert_ne!(
        f.call(
            &token,
            "/v1/labs/workers/inputs/file",
            f.read(&attempt, &f.used.id)
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_revoked_key_or_lost_membership_refuses_reads() {
    let f = Fixture::new(false).await;
    let (token, id, attempt) = f.started("server").await;
    f.auth.allowed.store(false, Ordering::SeqCst);
    assert_eq!(
        f.json(&token, "/v1/labs/workers/inputs", attempt.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    f.auth.allowed.store(true, Ordering::SeqCst);
    assert_eq!(
        f.json(&token, "/v1/labs/workers/inputs", attempt.clone())
            .await
            .0,
        StatusCode::OK
    );
    let r = Request::builder()
        .method("DELETE")
        .uri(format!("/v1/labs/workers/{id}"))
        .header("authorization", "Bearer alice")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        f.app.clone().oneshot(r).await.unwrap().status(),
        StatusCode::OK
    );
    assert_ne!(
        f.call(
            &token,
            "/v1/labs/workers/inputs/file",
            f.read(&attempt, &f.used.id)
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_private_referenced_file_keeps_the_job_off_cloud_and_server_workers() {
    let f = Fixture::new(true).await;
    let (token, _, attempt) = f.started("enrolled").await;
    let (s, inputs) = f.json(&token, "/v1/labs/workers/inputs", attempt).await;
    assert_eq!(s, StatusCode::OK, "{inputs}");
    // The private mesh stays on the enrolled host; only the public file is served.
    let files: Vec<_> = inputs["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(files, [f.used.id.as_str()]);

    let f = Fixture::new(true).await;
    let (token, _, attempt) = f.started("cloud").await;
    let (s, body) = f.json(&token, "/v1/labs/workers/inputs", attempt).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "private_local");
}
