use super::*;
use axum::{body::to_bytes, routing::any};
use coder_environment::{
    ArtifactPin, Environment, ImagePin, Limits, Platform, ProjectLink, Qualification, Recipe,
    Script, SourcePin,
};
use coder_environment_operator::{
    Providers,
    studio::{Config as StudioConfig, Studio, Transports},
};
use coder_working_computer::{Bounds, Computer, Principal, Spec, provider::fake::FakeProvider};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;
const SESSION: &str = "sess_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
#[derive(Clone)]
struct NoModel;
impl codex_transport::Transport for NoModel {
    async fn respond(
        &self,
        _: &codex_transport::Request,
    ) -> Result<codex_transport::Reply, codex_transport::TransportError> {
        Err(codex_transport::TransportError::Exhausted)
    }
}
struct Native {
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    created_at: u64,
    expires_at: u64,
    active: AtomicBool,
    epoch: AtomicU64,
    repo: AtomicBool,
    tickets: AtomicUsize,
}
async fn native(State(n): State<Arc<Native>>, request: Request) -> Response {
    let path = request.uri().path();
    if path == oa_auth::repos::broker::PATH {
        return if n.repo.load(Ordering::SeqCst) {
            (
                StatusCode::OK,
                "username=x-access-token\npassword=fixture-repository-secret\n",
            )
                .into_response()
        } else {
            StatusCode::FORBIDDEN.into_response()
        };
    }
    if !n.active.load(Ordering::SeqCst)
        || request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            != Some(&format!("Bearer {SESSION}"))
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match path {
        "/v1/session" => {
            let observed = (n.clock)();
            Json(json!({"session":{
                "id":"session-alice","kind":"user","account":"alice",
                "created_at":n.created_at,"expires_at":n.expires_at,
                "state":if observed < n.expires_at { "active" } else { "expired" }
            }})).into_response()
        },
        "/v1/account"=>Json(json!({"account":{"id":"alice","label":"Alice"},"workspaces":[{"id":"alice-workspace","name":"Alice work","role":"owner"}]})).into_response(),
        "/v1/workspaces/alice-workspace"=>Json(json!({"workspace":{"id":"alice-workspace","tenant":"fixture","members_epoch":n.epoch.load(Ordering::SeqCst)},"role":"owner"})).into_response(),
        "/v1/account/github/broker"=>{n.tickets.fetch_add(1,Ordering::SeqCst);Json(json!({"ticket":"ogb_fixture-private-ticket"})).into_response()},
        _=>StatusCode::NOT_FOUND.into_response(),
    }
}
fn config(state: &Path) -> StudioConfig {
    StudioConfig {
        schema: coder_environment_operator::studio::SCHEMA.into(),
        state: state.into(),
        machines: coder_environment_operator::Config {
            schema: coder_environment_operator::SCHEMA.into(),
            provider: coder_environment_operator::ProviderKind::Boat,
            gce: None,
            workdir: "/workspace".into(),
            template: Some("fixture-base".into()),
            credential_names: BTreeSet::new(),
            tick_seconds: 15,
        },
        owner: Principal {
            workspace: "fixture".into(),
            principal: "operator".into(),
        },
        codex_home: Some(state.join("codex-home")),
        model: None,
        size: None,
        deadline_seconds: None,
        github_token: None,
        claude_key: None,
        model_api: None,
        home_retention_days: None,
    }
}
fn studio(state: &Path) -> Arc<Studio> {
    let provider = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let transports: Transports<NoModel> = Arc::new(|_| Ok(NoModel));
    Studio::start(
        config(state),
        "fixture-base",
        Providers {
            setup: provider.clone(),
            build: provider.clone(),
            verify: provider,
        },
        Arc::new(|_| Ok(coder_environment::evidence::Redactor::new())),
        transports,
        std::time::Duration::from_millis(10),
    )
    .unwrap()
}
struct Fixture {
    dir: tempfile::TempDir,
    app: App,
    binding: Binding,
    native: Arc<Native>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    async fn new() -> Self {
        Self::with_clock(Arc::new(now)).await
    }
    async fn with_clock(clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        // Authentication reads the same session twice; its timestamps stay fixed.
        let issued_at = clock();
        let native_state = Arc::new(Native {
            clock,
            created_at: issued_at - 1,
            expires_at: issued_at + 3600,
            active: AtomicBool::new(true),
            epoch: AtomicU64::new(3),
            repo: AtomicBool::new(true),
            tickets: AtomicUsize::new(0),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new()
            .fallback(native)
            .with_state(native_state.clone());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        // The private configuration is opened without following links, so
        // name it by its real path (macOS keeps temporary folders under the
        // `/var` link).
        let private = dir.path().canonicalize().unwrap();
        let csrf = private.join("csrf");
        std::fs::write(&csrf, [9; 32]).unwrap();
        std::fs::set_permissions(&csrf, std::fs::Permissions::from_mode(0o600)).unwrap();
        let path = private.join("cloud.json");
        std::fs::write(&path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":endpoint,"csrf_secret":csrf})).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let root = studio(&dir.path().join("owners"));
        let child = studio(&dir.path().join("owners/public-computers"));
        root.attach_public(child.clone()).unwrap();
        let mut config = crate::Config::development(dir.path().join("tasks"));
        config.ask_salt = [7; 32];
        config.agent_accounts = vec!["alice".into()];
        config.cloud = Some(Arc::new(CloudSession::load(&path).unwrap()));
        config.api_host = Some("api.fixture.test".into());
        config.inference = Some(Arc::new(crate::upstream::Upstream::new(&endpoint).unwrap()));
        config.solver_keys = Some(Arc::new(
            oa_seal::Keyring::scratch("environment-test").unwrap().0,
        ));
        config.environments.set(root);
        let app = App(Arc::new(crate::Inner { config }));
        let store = store(&app).unwrap();
        let layout = coder_environment_operator::Layout::under(&store.state);
        let project = ProjectLink {
            workspace: "fixture".into(),
            project: "repo".into(),
        };
        let source = SourcePin {
            repository: Some("example/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        };
        let recipe = Recipe {
            schema: coder_environment::RECIPE_SCHEMA.into(),
            base: ImagePin {
                provider: coder_environment::Provider::Boat,
                image_id: "fixture-base".into(),
                digest: "c".repeat(64),
            },
            runtime: ArtifactPin {
                revision: "fixture".into(),
                digest: "d".repeat(64),
            },
            platform: Platform {
                os: "linux".into(),
                architecture: "x86_64".into(),
            },
            install: Script {
                cwd: ".".into(),
                digest: "e".repeat(64),
            },
            start: Default::default(),
            inputs: Default::default(),
            credential_names: Default::default(),
            qualification: Qualification {
                profile: "fixture".into(),
                plan_digest: "f".repeat(64),
            },
            limits: Limits {
                deadline_seconds: 3600,
                concurrent_machines: 2,
                total_machine_allocations: 4,
                output_bytes: 1048576,
            },
            capture: Default::default(),
        };
        let env = Environment::new(
            "envc-fixture",
            project.clone(),
            source.clone(),
            recipe,
            coder_environment_operator::now_ms(),
        )
        .unwrap();
        coder_environment::store::Store::under(layout.environments())
            .create(&env)
            .unwrap();
        shared::write(
            &store
                .state
                .join("environment-studio/envc-fixture/meta.json"),
            &json!({"account":"alice","public_computers":true}),
        )
        .unwrap();
        let resolved = Resolved {
            repository: coder_environment_operator::studio::github::RepoName {
                owner: "example".into(),
                name: "repo".into(),
            },
            branch: "main".into(),
            commit: source.revision.clone(),
            private: true,
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:4300".parse().unwrap());
        headers.insert(
            header::COOKIE,
            format!("oa_cloud_session={SESSION}; oa_cloud_workspace=alice-workspace")
                .parse()
                .unwrap(),
        );
        let auth = Authorization::prepare(&app, &headers, &resolved)
            .await
            .unwrap();
        auth.save(&app, "envc-fixture").unwrap();
        let c = Computer::for_setup(
            Spec {
                id: "native-fixture".into(),
                chat: "setup-fixture".into(),
                owner: Principal {
                    workspace: "fixture".into(),
                    principal: "operator".into(),
                },
                project,
                source,
                base: None,
                size: "small".into(),
                credential_names: Default::default(),
                services: vec![],
                bounds: Bounds {
                    idle_ms: 60000,
                    observed_extension_ms: 60000,
                    absolute_ms: 3600000,
                },
            },
            "envc-fixture",
            coder_environment_operator::now_ms(),
        )
        .unwrap();
        coder_working_computer::store::Store::under(layout.computers())
            .create(&c)
            .unwrap();
        let runtime = shared::Runtime::new(&store.state, "http://127.0.0.1:4300", [7; 32]).unwrap();
        let binding = runtime.prepare(&c, "create-fixture").unwrap();
        Self {
            dir,
            app,
            binding,
            native: native_state,
            server,
        }
    }
    fn request(&self, method: &str, path: &str, body: Value) -> Request {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "api.fixture.test")
            .header(
                header::AUTHORIZATION,
                format!(
                    "Bearer {}",
                    shared::credential(&self.binding, &self.app.config.ask_salt)
                ),
            )
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }
}
#[tokio::test]
async fn authorization_keeps_session_expiry_when_clock_advances_between_reads() {
    for step in [1, 60] {
        let initial = now() - 60;
        let clock = Arc::new(AtomicU64::new(initial));
        let observed = clock.clone();
        let f =
            Fixture::with_clock(Arc::new(move || observed.fetch_add(step, Ordering::SeqCst))).await;
        assert!(clock.load(Ordering::SeqCst) >= initial + 3 * step);
        let grant = store(&f.app)
            .unwrap()
            .grant("envc-fixture")
            .unwrap()
            .unwrap();
        assert_eq!(grant.expires_unix, initial + 3600);
        assert!(authorization(&f.app, &f.binding).await.is_ok());
        clock.store(initial + 3600, Ordering::SeqCst);
        assert!(authorization(&f.app, &f.binding).await.is_err());
    }
}

#[tokio::test]
async fn a_job_bootstraps_on_the_api_host_without_a_computer_and_excludes_account_routes() {
    let f = Fixture::new().await;
    let router = crate::router(f.app.config.clone());
    let r = router
        .clone()
        .oneshot(f.request("GET", "/v1/environment-computers/session", Value::Null))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
    let bytes = to_bytes(r.into_body(), 8192).await.unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(SESSION));
    assert!(String::from_utf8_lossy(&bytes).contains("alice"));
    for path in [
        "/v1/account",
        "/v1/computers",
        "/v1/environment-computers/another-attempt/image",
    ] {
        let r = router
            .clone()
            .oneshot(f.request("GET", path, Value::Null))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "{path}");
    }
    let bytes = std::fs::read(
        store(&f.app)
            .unwrap()
            .state
            .join("environment-computers/grants/envc-fixture.json"),
    )
    .unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains(SESSION));
    assert!(!text.contains("ogb_fixture-private-ticket"));
}
#[tokio::test]
async fn current_workspace_session_and_repository_revocation_stop_before_effects() {
    let f = Fixture::new().await;
    let effects = Arc::new(AtomicUsize::new(0));
    let seen = effects.clone();
    let router = Router::new()
        .fallback(any(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            async { StatusCode::OK }
        }))
        .layer(axum::middleware::from_fn_with_state(f.app.clone(), guard));
    let body = serde_json::to_value(&f.binding.options).unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(f.request("POST", "/v1/computers", body.clone()))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(f.native.tickets.load(Ordering::SeqCst), 1);
    f.native.epoch.store(4, Ordering::SeqCst);
    assert_eq!(
        router
            .clone()
            .oneshot(f.request("POST", "/v1/computers", body.clone()))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    f.native.epoch.store(3, Ordering::SeqCst);
    f.native.repo.store(false, Ordering::SeqCst);
    assert_eq!(
        router
            .clone()
            .oneshot(f.request("POST", "/v1/computers", body.clone()))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    f.native.repo.store(true, Ordering::SeqCst);
    f.native.active.store(false, Ordering::SeqCst);
    assert_eq!(
        router
            .oneshot(f.request("POST", "/v1/computers", body))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    assert_eq!(f.native.tickets.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn changing_the_account_or_source_does_not_relabel_a_grant() {
    let f = Fixture::new().await;
    let s = store(&f.app).unwrap();
    let mut grant = s.grant("envc-fixture").unwrap().unwrap();
    grant.account = "bob".into();
    s.save_grant("envc-fixture", &grant).unwrap();
    assert!(authorization(&f.app, &f.binding).await.is_err());
    let mut forged = f.binding.clone();
    forged.account = "bob".into();
    let original = shared::credential(&f.binding, &f.app.config.ask_salt);
    s.save(&forged).unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {original}").parse().unwrap(),
    );
    assert!(resolve(&f.app, &headers).await.is_err());
}

#[tokio::test]
async fn builders_and_verifiers_cannot_accept_unplanned_file_writes() {
    let f = Fixture::new().await;
    let mut b = f.binding.clone();
    let id = "cmp_0123456789abcdef0123";
    let path = format!("/v1/computers/{id}/files");
    assert_eq!(policy(&b, Some(id), "PUT", &path, &Value::Null), Some(true));
    for role in [
        shared::EnvironmentRole::Builder,
        shared::EnvironmentRole::Verifier,
    ] {
        b.attempt.role = role;
        assert_eq!(policy(&b, Some(id), "PUT", &path, &Value::Null), None);
        assert_eq!(policy(&b, Some(id), "GET", &path, &Value::Null), Some(true));
    }
}

#[tokio::test]
async fn retry_renews_authority_without_changing_an_older_attempts_epoch() {
    let f = Fixture::new().await;
    f.native.epoch.store(4, Ordering::SeqCst);
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, "127.0.0.1:4300".parse().unwrap());
    headers.insert(
        header::COOKIE,
        format!("oa_cloud_session={SESSION}; oa_cloud_workspace=alice-workspace")
            .parse()
            .unwrap(),
    );
    refresh(&f.app, &headers, "envc-fixture").await.unwrap();
    assert_eq!(
        store(&f.app)
            .unwrap()
            .grant("envc-fixture")
            .unwrap()
            .unwrap()
            .workspace,
        Some(("alice-workspace".into(), 4))
    );
    assert!(authorization(&f.app, &f.binding).await.is_err());
}

#[tokio::test]
async fn duplicate_and_malformed_job_bearers_never_reach_the_account_gateway() {
    let f = Fixture::new().await;
    let router = crate::router(f.app.config.clone());
    let mut request = f.request("GET", "/v1/account", Value::Null);
    request
        .headers_mut()
        .append(header::AUTHORIZATION, "Bearer another".parse().unwrap());
    assert_eq!(
        router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut request = f.request("GET", "/v1/account", Value::Null);
    let token = shared::credential(&f.binding, &f.app.config.ask_salt);
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("bearer {token}").parse().unwrap(),
    );
    assert_eq!(
        router.oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}
