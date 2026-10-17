use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use lab_api::{Access, Action, Api, ApiError, Authorizer, Gate, KindAdapter, Registry};
use lab_jobs::{
    authority::{Authority, Enrolled},
    protocol,
};
use lab_kit::{LabKind, SpaceRef, Want, echo::Echo};
use lab_store::{CheckedDraft, LabStore, Store, backend::FsBackend};
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
        let a = self.authorize(h, "me", Action::Edit).await?;
        self.credentials.revoke(&a.account, id).await?;
        Ok(())
    }
}
struct Fixture {
    app: Router,
    auth: Arc<Auth>,
    _tmp: tempfile::TempDir,
    job: String,
}
impl Fixture {
    async fn new() -> Self {
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
                Arc::new(KindAdapter::<Echo>::new(|_, _| {
                    Err(ApiError::invalid("This tool isn't supported."))
                })),
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
        let version = store
            .create(
                "d",
                "Echo",
                "alice",
                CheckedDraft::initial::<Echo>(
                    "echo.v1",
                    json!({"text":"hello"}),
                    "Echo",
                    &Value::Null,
                )
                .unwrap(),
                now(),
            )
            .await
            .unwrap();
        let job = store
            .create_job(
                Echo::jobs_for(&version, &Want::Card).remove(0),
                &Echo::manifest(),
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
            _tmp: tmp,
            job,
        }
    }
    async fn call(
        &self,
        token: &str,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let r = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let r = self.app.clone().oneshot(r).await.unwrap();
        let status = r.status();
        let bytes = r.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
    async fn enroll(&self) -> (String, String) {
        let (s,b)=self.call("alice","POST","/v1/labs/workers",json!({"space":"me","lifetime_ms":60000,"capabilities":{
            "worker":"fixture","placement":"enrolled","cpu_millis":1000,"memory_bytes":134217728,
            "gpu":false,"tools":[],"jobs":["echo/shout"],"local_assets":[],"max_seconds":600,
        }})).await;
        assert_eq!(s, StatusCode::OK);
        (
            b["token"].as_str().unwrap().into(),
            b["worker"]["id"].as_str().unwrap().into(),
        )
    }
}
#[tokio::test]
async fn a_job_key_cannot_read_documents_or_choose_another_space_or_attempt() {
    let f = Fixture::new().await;
    let (token, _) = f.enroll().await;
    assert_eq!(
        f.call(&token, "GET", "/v1/labs/documents", Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            &token,
            "POST",
            "/v1/labs/workers/poll",
            json!({"space":"project:other"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, work) = f
        .call(&token, "POST", "/v1/labs/workers/poll", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(work["work"]["assignment"]["lease"]["job"]["id"], f.job);
    assert_ne!(
        f.call(
            &token,
            "POST",
            "/v1/labs/workers/start",
            json!({"job":"another-job","attempt":1})
        )
        .await
        .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn worker_revocation_and_current_membership_stop_heartbeats() {
    let f = Fixture::new().await;
    let (token, id) = f.enroll().await;
    let (_, work) = f
        .call(&token, "POST", "/v1/labs/workers/poll", json!({}))
        .await;
    let attempt = json!({"job":f.job,"attempt":work["work"]["assignment"]["lease"]["attempt"]});
    assert_eq!(
        f.call(&token, "POST", "/v1/labs/workers/start", attempt.clone())
            .await
            .0,
        StatusCode::OK
    );
    f.auth.allowed.store(false, Ordering::SeqCst);
    assert_eq!(
        f.call(
            &token,
            "POST",
            "/v1/labs/workers/heartbeat",
            attempt.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    f.auth.allowed.store(true, Ordering::SeqCst);
    assert_eq!(
        f.call(
            "bob",
            "DELETE",
            &format!("/v1/labs/workers/{id}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        f.call(
            "alice",
            "DELETE",
            &format!("/v1/labs/workers/{id}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_ne!(
        f.call(&token, "POST", "/v1/labs/workers/heartbeat", attempt)
            .await
            .0,
        StatusCode::OK
    );
}
