use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use lab_api::*;
use lab_kit::{echo::Echo, *};
use lab_store::backend::FsBackend;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

struct Auth;
#[async_trait::async_trait]
impl Authorizer for Auth {
    async fn authorize(
        &self,
        h: &HeaderMap,
        space: &str,
        action: Action,
    ) -> Result<Access, ApiError> {
        if h.get_all("authorization").iter().count() != 1 {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "signed_out",
                "Sign in to continue.",
            ));
        }
        let token = h
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or_default();
        if !["alice", "bob", "viewer", "editor", "closed"].contains(&token) {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "signed_out",
                "Sign in to continue.",
            ));
        }
        let target = if space == "me" {
            SpaceRef::Personal {
                account: token.into(),
            }
        } else if space == "project:shared" && token != "bob" {
            if token == "viewer" && action != Action::Read
                || token == "editor" && action == Action::Delete
            {
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "project_role",
                    "This action needs a different project role.",
                ));
            }
            SpaceRef::Project {
                project: "shared".into(),
            }
        } else {
            return Err(ApiError::missing());
        };
        Ok(Access {
            account: token.into(),
            space: target,
            experimental: token != "closed",
        })
    }
}
fn echo_tool(name: &str, args: &Value) -> Result<Tool, ApiError> {
    if name != "echo_set" {
        return Err(ApiError::missing());
    }
    let document = args["document"]
        .as_str()
        .ok_or_else(|| ApiError::invalid("Choose a document."))?
        .into();
    let text = args["text"]
        .as_str()
        .ok_or_else(|| ApiError::invalid("Enter text."))?;
    Ok(Tool::Change {
        document,
        change: Change::Intent {
            intent: json!({"intent":"set","text":text}),
        },
        summary: "Tool edit".into(),
    })
}
fn app() -> (Router, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let backend = Arc::new(FsBackend::open(root.path()).unwrap());
    let mut registry = Registry::default();
    registry
        .register(
            Arc::new(KindAdapter::<Echo>::new(echo_tool)),
            Gate::Experimental,
        )
        .unwrap();
    let api = Arc::new(Api {
        backend,
        authorizer: Arc::new(Auth),
        registry,
    });
    (api.router(), root)
}
async fn call(
    app: &Router,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
    extra: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        request = request.header("Authorization", format!("Bearer {t}"));
    }
    if body.is_some() {
        request = request.header("Content-Type", "application/json");
    }
    for (k, v) in extra {
        request = request.header(*k, *v);
    }
    let request = request
        .body(Body::from(
            body.map(|b| serde_json::to_vec(&b).unwrap())
                .unwrap_or_default(),
        ))
        .unwrap();
    let r = app.clone().oneshot(request).await.unwrap();
    let status = r.status();
    let headers = r.headers().clone();
    let bytes = r.into_body().collect().await.unwrap().to_bytes().to_vec();
    if let Ok(body) = serde_json::from_slice::<Value>(&bytes) {
        if let Some(message) = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
        {
            assert!(
                oa_copy::violations(message, &[]).is_empty(),
                "{message}: {:?}",
                oa_copy::violations(message, &[])
            );
        }
    }
    (status, headers, bytes)
}
fn value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}
fn doc(id: &str, space: &str) -> Value {
    json!({"space":space,"id":id,"kind":"echo","schema":"echo.v1","title":"Synthetic","content":{"text":"first"},"summary":"Created"})
}
fn change(base: u64, text: &str, space: &str) -> Value {
    json!({"space":space,"base":base,"change":{"type":"intent","intent":{"intent":"set","text":text}},"summary":"Edited"})
}

#[tokio::test]
async fn every_route_conflicts_refusals_and_cleanup() {
    let (app, _root) = app();
    let (s, _, b) = call(&app, None, "GET", "/v1/labs/kinds", None, &[]).await;
    assert_eq!(s, 401);
    assert_eq!(value(&b)["error"]["code"], "signed_out");
    let (s, _, b) = call(&app, Some("alice"), "GET", "/v1/labs/kinds", None, &[]).await;
    assert_eq!(s, 200);
    assert_eq!(value(&b)["data"][0]["kind"], "echo");
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/kinds/echo/tools",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/kinds/missing/tools",
            None,
            &[]
        )
        .await
        .0,
        404
    );
    let (s, _, b) = call(
        &app,
        Some("alice"),
        "POST",
        "/v1/labs/documents",
        Some(doc("d", "me")),
        &[],
    )
    .await;
    assert_eq!(s, 201);
    assert_eq!(value(&b)["author"], "alice");
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents?kind=echo&limit=1",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/d",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents/d/versions",
            Some(change(1, "second", "me")),
            &[]
        )
        .await
        .0,
        201
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents/d/versions",
            Some(change(1, "stale", "me")),
            &[]
        )
        .await
        .0,
        409
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents/d/versions",
            Some(change(2, "", "me")),
            &[]
        )
        .await
        .0,
        422
    );
    let (s, _, b) = call(
        &app,
        Some("alice"),
        "GET",
        "/v1/labs/documents/d/versions",
        None,
        &[],
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(value(&b)["data"].as_array().unwrap().len(), 2);
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/d/versions/2",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/d/versions/2/card",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/jobs?document=d",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    let j = json!({"version":{"document":"d","version":2},"want":{"want":"card"}});
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/jobs",
            Some(j.clone()),
            &[]
        )
        .await
        .0,
        400
    );
    let (s, _, b) = call(
        &app,
        Some("alice"),
        "POST",
        "/v1/labs/jobs",
        Some(j.clone()),
        &[("Idempotency-Key", "job-one")],
    )
    .await;
    assert_eq!(s, 201);
    let job = value(&b);
    let id = job["id"].as_str().unwrap();
    let (_, _, retry) = call(
        &app,
        Some("alice"),
        "POST",
        "/v1/labs/jobs",
        Some(j),
        &[("Idempotency-Key", "job-one")],
    )
    .await;
    assert_eq!(value(&retry), job);
    let another = json!({"version":{"document":"d","version":1},"want":{"want":"card"}});
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/jobs",
            Some(another),
            &[("Idempotency-Key", "job-one")]
        )
        .await
        .0,
        409
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            &format!("/v1/labs/jobs/{id}"),
            None,
            &[]
        )
        .await
        .0,
        200
    );
    // Reading cards, versions, and job lists never takes a queued job.
    let (_, _, b) = call(
        &app,
        Some("alice"),
        "GET",
        &format!("/v1/labs/jobs/{id}"),
        None,
        &[],
    )
    .await;
    assert_eq!(value(&b)["status"], "queued");
    let up = json!({"version":{"document":"d","version":2},"media_type":"text/plain","class":"no_training","content_base64":"YWJjZGVm"});
    let (s, _, b) = call(
        &app,
        Some("alice"),
        "POST",
        "/v1/labs/uploads",
        Some(up),
        &[],
    )
    .await;
    assert_eq!(s, 201);
    let artifact = value(&b);
    assert_eq!(artifact["no_training"], true);
    let aid = artifact["id"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            &format!("/v1/labs/artifacts/{aid}"),
            None,
            &[]
        )
        .await
        .0,
        200
    );
    let (s, h, b) = call(
        &app,
        Some("alice"),
        "GET",
        &format!("/v1/labs/artifacts/{aid}/content"),
        None,
        &[("Range", "bytes=1-3")],
    )
    .await;
    assert_eq!(s, 206);
    assert_eq!(b, b"bcd");
    assert_eq!(h["content-range"], "bytes 1-3/6");
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            &format!("/v1/labs/artifacts/{aid}/content"),
            None,
            &[("Range", "bytes=8-9")]
        )
        .await
        .0,
        416
    );
    let note = json!({"version":2,"locator":{"at":"frame","frame":2},"text":"Synthetic note"});
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents/d/notes",
            Some(note),
            &[]
        )
        .await
        .0,
        201
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/d/notes",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    let (s, _, b) = call(
        &app,
        Some("alice"),
        "GET",
        "/v1/labs/documents/d/events?since=0",
        None,
        &[],
    )
    .await;
    assert_eq!(s, 200);
    assert!(value(&b)["data"].as_array().unwrap().len() >= 5);
    let mut tool = json!({"base":2,"arguments":{"document":"d","text":"from tool"}});
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/kinds/echo/tools/echo_set",
            Some(tool.clone()),
            &[]
        )
        .await
        .0,
        200
    );
    // Reusing the same base cannot apply a mutating tool twice.
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/kinds/echo/tools/echo_set",
            Some(tool.clone()),
            &[]
        )
        .await
        .0,
        409
    );
    tool["base"] = json!(3);
    tool["arguments"]["text"] = json!("");
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/kinds/echo/tools/echo_set",
            Some(tool),
            &[]
        )
        .await
        .0,
        422
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "DELETE",
            &format!("/v1/labs/jobs/{id}"),
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "DELETE",
            "/v1/labs/documents/d",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/d",
            None,
            &[]
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            &format!("/v1/labs/artifacts/{aid}/content"),
            None,
            &[]
        )
        .await
        .0,
        404
    );
}

#[tokio::test]
async fn account_isolation_project_roles_and_admission() {
    let (app, _root) = app();
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents",
            Some(doc("personal", "me")),
            &[]
        )
        .await
        .0,
        201
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents",
            Some(doc("shared", "project:shared")),
            &[]
        )
        .await
        .0,
        201
    );
    for path in [
        "/v1/labs/documents/personal",
        "/v1/labs/documents/personal/versions",
        "/v1/labs/documents/personal/versions/1",
        "/v1/labs/documents/personal/versions/1/card",
        "/v1/labs/documents/personal/notes",
        "/v1/labs/documents/personal/events",
        "/v1/labs/jobs?document=personal",
    ] {
        assert_eq!(
            call(&app, Some("bob"), "GET", path, None, &[]).await.0,
            404,
            "{path}"
        );
    }
    assert_eq!(
        call(
            &app,
            Some("bob"),
            "POST",
            "/v1/labs/documents/personal/versions",
            Some(change(1, "bad", "me")),
            &[]
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            Some("bob"),
            "DELETE",
            "/v1/labs/documents/personal",
            None,
            &[]
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/personal?space=project:shared",
            None,
            &[]
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            Some("viewer"),
            "GET",
            "/v1/labs/documents/shared?space=project:shared",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            Some("viewer"),
            "POST",
            "/v1/labs/documents/shared/versions",
            Some(change(1, "bad", "project:shared")),
            &[]
        )
        .await
        .0,
        403
    );
    assert_eq!(
        call(
            &app,
            Some("editor"),
            "POST",
            "/v1/labs/documents/shared/versions",
            Some(change(1, "allowed", "project:shared")),
            &[]
        )
        .await
        .0,
        201
    );
    assert_eq!(
        call(
            &app,
            Some("editor"),
            "DELETE",
            "/v1/labs/documents/shared?space=project:shared",
            None,
            &[]
        )
        .await
        .0,
        403
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents?space=account:bob",
            None,
            &[]
        )
        .await
        .0,
        400
    );
    let (s, _, b) = call(&app, Some("closed"), "GET", "/v1/labs/kinds", None, &[]).await;
    assert_eq!(s, 200);
    assert_eq!(value(&b)["data"], json!([]));
    assert_eq!(
        call(
            &app,
            Some("closed"),
            "POST",
            "/v1/labs/documents",
            Some(doc("denied", "me")),
            &[]
        )
        .await
        .0,
        403
    );
    let mut forged = doc("forged", "me");
    forged["account"] = json!("bob");
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents",
            Some(forged),
            &[]
        )
        .await
        .0,
        422
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents?limit=0",
            None,
            &[]
        )
        .await
        .0,
        400
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents?after=bad",
            None,
            &[]
        )
        .await
        .0,
        400
    );
}
#[test]
fn contract_has_unique_operations_and_generated_request_fields() {
    let spec = contract::document();
    let fixture: Value =
        serde_json::from_str(include_str!("../../../docs/sdk/openapi.labs.json")).unwrap();
    assert_eq!(
        spec, fixture,
        "Regenerate with cargo run -p lab-api --example openapi"
    );
    let mut ids = std::collections::BTreeSet::new();
    for (path, method, id, _, _) in contract::OPERATIONS {
        let op = &spec["paths"][path][method];
        assert_eq!(op["operationId"], *id);
        assert!(ids.insert(*id));
        assert!(op["responses"]["401"].is_object());
    }
    assert_eq!(
        spec["components"]["schemas"]["LabCreateDocument"]["properties"]["space"]["default"],
        "me"
    );
    assert_eq!(
        spec["components"]["schemas"]["LabCreateDocument"]["additionalProperties"],
        false
    );
}

struct ReferenceKind;
impl LabKind for ReferenceKind {
    const KIND: &'static str = "reference";
    fn manifest() -> KindManifest {
        let mut m = Echo::manifest();
        m.kind = Self::KIND.into();
        m.schemas = vec!["reference.v1".into()];
        m
    }
    fn check(content: &Value, m: &Measurements) -> Result<Report, Refusal> {
        let _: Vec<ResourceReference> = serde_json::from_value(content["references"].clone())
            .map_err(|_| Refusal::new("references", "Choose valid document references."))?;
        Echo::check(&json!({"text":content["text"]}), m)
    }
    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        let content = match change {
            Change::Intent { intent } => intent.clone(),
            _ => return Err(Refusal::new("intent", "Choose a document.")),
        };
        let report = Self::check(&content, m)?;
        let _ = base;
        Ok(Draft {
            content,
            summary: summary.into(),
            report,
            assets: vec![],
        })
    }
    fn jobs_for(_: &Version, _: &Want) -> Vec<JobSpec> {
        vec![]
    }
    fn card(v: &Version, j: &[JobState], a: &[Artifact]) -> Card {
        let mut c = Echo::card(v, j, a);
        c.kind = Self::KIND.into();
        c
    }
}
struct ReferenceRuntime(KindAdapter<ReferenceKind>);
impl lab_api::Kind for ReferenceRuntime {
    fn manifest(&self) -> KindManifest {
        self.0.manifest()
    }
    fn initial(
        &self,
        s: &str,
        c: Value,
        summary: &str,
    ) -> lab_store::Result<lab_store::CheckedDraft> {
        self.0.initial(s, c, summary)
    }
    fn change(
        &self,
        v: &Version,
        c: &Change,
        summary: &str,
    ) -> lab_store::Result<lab_store::CheckedDraft> {
        self.0.change(v, c, summary)
    }
    fn jobs(&self, v: &Version, w: &Want) -> Vec<JobSpec> {
        self.0.jobs(v, w)
    }
    fn card(&self, v: &Version, j: &[JobState], a: &[Artifact]) -> Card {
        self.0.card(v, j, a)
    }
    fn tool(&self, n: &str, a: &Value) -> Result<Tool, ApiError> {
        self.0.tool(n, a)
    }
    fn references(&self, c: &Value) -> Result<Vec<ResourceReference>, Refusal> {
        serde_json::from_value(c["references"].clone())
            .map_err(|_| Refusal::new("references", "Choose valid document references."))
    }
}
struct Revocable(Arc<std::sync::atomic::AtomicBool>);
#[async_trait::async_trait]
impl Authorizer for Revocable {
    async fn authorize(&self, h: &HeaderMap, s: &str, a: Action) -> Result<Access, ApiError> {
        if s == "project:shared" && self.0.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ApiError::missing());
        }
        Auth.authorize(h, s, a).await
    }
}
#[tokio::test]
async fn references_check_both_spaces_and_kinds_on_writes_and_reads() {
    let root = tempfile::tempdir().unwrap();
    let revoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut registry = Registry::default();
    registry
        .register(
            Arc::new(KindAdapter::<Echo>::new(echo_tool)),
            Gate::Experimental,
        )
        .unwrap();
    registry
        .register(
            Arc::new(ReferenceRuntime(KindAdapter::new(echo_tool))),
            Gate::Experimental,
        )
        .unwrap();
    let api = Arc::new(Api {
        backend: Arc::new(FsBackend::open(root.path()).unwrap()),
        authorizer: Arc::new(Revocable(revoked.clone())),
        registry,
    });
    let app = api.router();
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents",
            Some(doc("source", "project:shared")),
            &[]
        )
        .await
        .0,
        201
    );
    let request = json!({"id":"ref","kind":"reference","schema":"reference.v1","title":"Reference","content":{"text":"Linked","references":[{"space":"project:shared","version":{"document":"source","version":1}}]},"summary":"Linked"});
    assert_eq!(
        call(
            &app,
            Some("bob"),
            "POST",
            "/v1/labs/documents",
            Some(request.clone()),
            &[]
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents",
            Some(request),
            &[]
        )
        .await
        .0,
        201
    );
    assert_eq!(
        call(
            &app,
            Some("bob"),
            "GET",
            "/v1/labs/documents/ref/versions/1",
            None,
            &[]
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/ref/versions/1",
            None,
            &[]
        )
        .await
        .0,
        200
    );
    let wrong = json!({"space":"project:shared","base":1,"arguments":{"document":"source","text":"Wrong lab"}});
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/kinds/reference/tools/echo_set",
            Some(wrong),
            &[]
        )
        .await
        .0,
        403
    );
    revoked.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/ref/versions/1",
            None,
            &[]
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "GET",
            "/v1/labs/documents/ref/versions/1/card",
            None,
            &[]
        )
        .await
        .0,
        404
    );
}
#[tokio::test]
async fn event_stream_stops_when_project_access_is_revoked() {
    let root = tempfile::tempdir().unwrap();
    let revoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut registry = Registry::default();
    registry
        .register(
            Arc::new(KindAdapter::<Echo>::new(echo_tool)),
            Gate::Experimental,
        )
        .unwrap();
    let app = Arc::new(Api {
        backend: Arc::new(FsBackend::open(root.path()).unwrap()),
        authorizer: Arc::new(Revocable(revoked.clone())),
        registry,
    })
    .router();
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents",
            Some(doc("stream", "project:shared")),
            &[]
        )
        .await
        .0,
        201
    );
    let request = Request::builder()
        .uri("/v1/labs/documents/stream/events?space=project:shared")
        .header("Authorization", "Bearer viewer")
        .header("Accept", "text/event-stream")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);
    let mut body = response.into_body();
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("event: changes"));
    revoked.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), body.frame())
            .await
            .unwrap()
            .is_none()
    );
}

struct LimitedEcho;
impl LabKind for LimitedEcho {
    const KIND: &'static str = "echo";
    fn manifest() -> KindManifest {
        let mut m = Echo::manifest();
        m.limits.runs_per_hour = 0;
        m
    }
    fn check(c: &Value, m: &Measurements) -> Result<Report, Refusal> {
        Echo::check(c, m)
    }
    fn apply(v: &Version, c: &Change, s: &str, m: &Measurements) -> Result<Draft, Refusal> {
        Echo::apply(v, c, s, m)
    }
    fn jobs_for(v: &Version, w: &Want) -> Vec<JobSpec> {
        Echo::jobs_for(v, w)
    }
    fn card(v: &Version, j: &[JobState], a: &[Artifact]) -> Card {
        Echo::card(v, j, a)
    }
}
#[tokio::test]
async fn hourly_limit_has_a_stable_code_and_retry_after() {
    let root = tempfile::tempdir().unwrap();
    let mut registry = Registry::default();
    registry
        .register(
            Arc::new(KindAdapter::<LimitedEcho>::new(echo_tool)),
            Gate::Experimental,
        )
        .unwrap();
    let app = Arc::new(Api {
        backend: Arc::new(FsBackend::open(root.path()).unwrap()),
        authorizer: Arc::new(Auth),
        registry,
    })
    .router();
    assert_eq!(
        call(
            &app,
            Some("alice"),
            "POST",
            "/v1/labs/documents",
            Some(doc("limited", "me")),
            &[]
        )
        .await
        .0,
        201
    );
    let r = json!({"version":{"document":"limited","version":1},"want":{"want":"card"}});
    let (s, h, b) = call(
        &app,
        Some("alice"),
        "POST",
        "/v1/labs/jobs",
        Some(r),
        &[("Idempotency-Key", "limited-job")],
    )
    .await;
    assert_eq!(s, 429);
    assert_eq!(h["retry-after"], "60");
    assert_eq!(value(&b)["error"]["code"], "rate_limited");
}
