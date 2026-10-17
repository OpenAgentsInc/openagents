use super::*;
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use lab_api::{Access, Action, Api, Authorizer, Gate, Registry};
use lab_kit::SpaceRef;
use lab_store::{ArtifactInput, backend::FsBackend};
use std::sync::Arc;
use tower::ServiceExt;

struct Alice;
#[async_trait::async_trait]
impl Authorizer for Alice {
    async fn authorize(&self, _: &HeaderMap, space: &str, _: Action) -> Result<Access, ApiError> {
        if space != "me" {
            return Err(ApiError::missing());
        }
        Ok(Access {
            account: "alice".into(),
            space: space_ref(),
            experimental: true,
            cad: false,
        })
    }
}
fn space_ref() -> SpaceRef {
    SpaceRef::Personal {
        account: "alice".into(),
    }
}
struct Fixture {
    app: Router,
    store: Store,
    _tmp: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::open(tmp.path().join("labs")).unwrap());
        let mut registry = Registry::default();
        registry
            .register(Arc::new(GameApi), Gate::Experimental)
            .unwrap();
        registry
            .register(Arc::new(ship()), Gate::Experimental)
            .unwrap();
        registry
            .register(Arc::new(video()), Gate::Experimental)
            .unwrap();
        let store = Store::new(backend.clone(), &space_ref()).unwrap();
        let app = Arc::new(Api {
            backend,
            authorizer: Arc::new(Alice),
            registry,
        })
        .router();
        Self {
            app,
            store,
            _tmp: tmp,
        }
    }
    async fn call(&self, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
        let r = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", "Bearer alice")
            .header("content-type", "application/json")
            .header("Idempotency-Key", lab_kit::digest(&json!([path, body])))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let r = self.app.clone().oneshot(r).await.unwrap();
        let status = r.status();
        let bytes = axum::body::to_bytes(r.into_body(), 4 << 20).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
    async fn create(&self, id: &str, kind: &str, schema: &str, content: Value) {
        let (s, b) = self
            .call(
                "POST",
                "/v1/labs/documents",
                json!({"id": id, "kind": kind, "schema": schema, "title": id,
                    "content": content, "summary": "Created"}),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
    }
    async fn tool(&self, kind: &str, name: &str, base: u64, args: Value) -> (StatusCode, Value) {
        self.call(
            "POST",
            &format!("/v1/labs/kinds/{kind}/tools/{name}"),
            json!({"base": base, "arguments": args}),
        )
        .await
    }
}

#[tokio::test]
async fn only_the_mapped_tools_are_offered() {
    let f = Fixture::new();
    let (s, kinds) = f.call("GET", "/v1/labs/kinds", Value::Null).await;
    assert_eq!(s, StatusCode::OK, "{kinds}");
    for (kind, tools) in [
        ("game", GAME_TOOLS),
        ("ship", SHIP_TOOLS),
        ("video", VIDEO_TOOLS),
    ] {
        let manifest = kinds["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["kind"] == kind)
            .unwrap();
        let names: Vec<_> = manifest["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, tools);
    }
}

#[tokio::test]
async fn a_game_edit_waits_for_its_playtest_then_saves() {
    let f = Fixture::new();
    f.create(
        "house",
        "game",
        game_lab::doc::KIND,
        game_lab::doc::original(),
    )
    .await;
    let patch = "[{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":6.2}]";
    let args = json!({"game": "house", "patch": patch, "summary": "A faster man"});
    let (s, b) = f.tool("game", "game_edit", 1, args.clone()).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    assert_eq!(b["error"]["code"], "playtest_waiting");
    assert_eq!(b["error"]["message"], PLAYTEST_WAITING);
    let jobs = f.store.jobs("house", 0, 10).await.unwrap().items;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].spec.job_type, "playtest");
    assert_eq!(jobs[0].status, JobStatus::Queued);
    assert!(jobs[0].message.as_deref().unwrap().contains("Waiting"));

    // Asking again doesn't queue a second playtest.
    assert_eq!(
        f.tool("game", "game_edit", 1, args.clone()).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(f.store.jobs("house", 0, 10).await.unwrap().items.len(), 1);

    // A worker plays it, as `game-lab playtest-job` does.
    let base = f.store.version("house", 1).await.unwrap();
    let lease = f
        .store
        .claim(&jobs[0].id, "worker", now(), 30_000)
        .await
        .unwrap();
    let lease = f.store.start(&lease, now()).await.unwrap();
    let input = json!({"work": {"version": base, "assignment": {"lease": lease}}});
    let output = game_lab::kind::playtest_handler(input.to_string().as_bytes()).unwrap();
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(output["outputs"][0]["content_base64"].as_str().unwrap())
        .unwrap();
    let artifact = f
        .store
        .put_job_artifact(
            &lease,
            ArtifactInput {
                version: base.reference(),
                role: ArtifactRole::Export,
                media_type: "application/json".into(),
                class: lab_kit::AssetClass::Public,
                no_training: false,
            },
            bytes,
            1 << 20,
            now(),
        )
        .await
        .unwrap();
    f.store
        .finish(&lease, JobStatus::Done, None, vec![artifact.id], now())
        .await
        .unwrap();

    let (s, b) = f.tool("game", "game_edit", 1, args).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["version"]["number"], 2);
    // A schema problem is refused at once, without a playtest.
    let (s, b) = f
        .tool(
            "game",
            "game_edit",
            2,
            json!({"game": "house", "patch": "[{\"op\":\"remove\",\"path\":\"/man/sight\"}]"}),
        )
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(b["error"]["code"], "game_schema");
}

#[tokio::test]
async fn ship_edits_are_checked_and_refusals_keep_the_ship() {
    let f = Fixture::new();
    let tug = serde_json::to_value(verse_lagrange2::ship::Spacecraft::sample()).unwrap();
    f.create("tug", "ship", verse_lagrange2::ship::SCHEMA, tug)
        .await;
    let (s, b) = f
        .tool(
            "ship",
            "ship_edit",
            1,
            json!({"doc": "tug", "edit": "scale", "parts": ["tank-port", "tank-starboard"],
                "factor": 1.05, "summary": "Both tanks 5% bigger"}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["version"]["number"], 2);
    let (s, b) = f
        .tool(
            "ship",
            "ship_edit",
            2,
            json!({"doc": "tug", "edit": "set_dry_mass", "parts": ["habitat"], "kg": 60000.0,
                "summary": "Heavy"}),
        )
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    assert_eq!(b["error"]["code"], "ship_check");
    assert_eq!(f.store.document("tug").await.unwrap().latest, 2);
}

#[tokio::test]
async fn video_edits_and_renders_through_the_platform() {
    let f = Fixture::new();
    let cut = json!({
        "fps": 30, "size": [320, 180], "out": "~/out.mp4", "timeline": {},
        "shots": [
            {"name": "open", "src": "~/clips/a.mp4", "in": 0, "dur": 1.0},
            {"name": "end", "src": "~/clips/b.mp4", "in": 0, "dur": 1.0}
        ]
    });
    f.create("film", "video", "cut.v1", cut).await;
    let (s, b) = f
        .tool(
            "video",
            "lab_edit",
            1,
            json!({"document": "film", "summary": "A longer end",
                "patch": "[{\"op\":\"replace\",\"path\":\"/shots/@end/dur\",\"value\":1.5}]"}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["version"]["number"], 2);
    let (s, b) = f
        .tool("video", "lab_render", 2, json!({"document": "film"}))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = f
        .tool(
            "video",
            "lab_render",
            2,
            json!({"document": "film", "version": "2"}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["spec"]["type"], "render", "{b}");
    assert_eq!(b["result"]["status"], "queued");
}
