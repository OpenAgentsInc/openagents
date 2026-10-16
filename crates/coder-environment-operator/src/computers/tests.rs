use super::*;
use axum::{
    Json, Router,
    extract::{Request, State},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use coder_working_computer::{
    Bounds, Principal, Spec,
    provider::{CommandCursor, CommandProgress, CommandSpec, Commands, Images, Outcome, Provider},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const ID: &str = "cmp_0123456789abcdef0123";
const COMMAND: &str = "trn_0123456789abcdef0123";

#[tokio::test]
async fn a_native_build_uses_its_job_key_and_keeps_its_environment_build_id() {
    use coder_working_computer::provider::fake::{FakeProvider, FakeRun};
    let f = Fixture::new().await;
    let provider = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let source = f.c.source.clone();
    provider.on_command(Box::new(move |spec, _, _| {
        if spec.id == "source" {
            let report = coder_environment_setup::source::Report::verified_for(&source, true);
            FakeRun::exit(0, &report.render(), "")
        } else {
            let mut run = FakeRun::exit(0, "", "");
            run.exit = None;
            run
        }
    }));
    let owners = crate::Owners::open(
        f.dir.path(),
        Providers {
            setup: provider.clone(),
            build: provider.clone(),
            verify: provider,
        },
        Arc::new(|_| Ok(coder_environment::evidence::Redactor::new())),
    )
    .unwrap();
    let script = b"./install.sh";
    std::fs::write(owners.layout.setup_blobs().join(digest(script)), script).unwrap();
    let job = owners
        .build(
            &coder_environment_build::service::BuildRequest {
                request_id: "native-build".into(),
                environment: "envc-fixture".into(),
                owner: f.c.owner.clone(),
                expected_draft_revision: 1,
                size: "small".into(),
                space: false,
            },
            crate::now_ms(),
        )
        .await
        .unwrap();
    assert_ne!(job.id, job.inputs.build_id);
    let c = Store::under(f.dir.path()).computer(&job.computer).unwrap();
    assert_eq!(c.chat, job.id);
    let runtime = Runtime::new(f.dir.path(), &f.origin, [7; 32]).unwrap();
    let mut wrong = c.clone();
    wrong.chat = job.inputs.build_id.clone();
    assert!(runtime.prepare(&wrong, "create").is_err());
    let binding = runtime.prepare(&c, "create").unwrap();
    assert_eq!(binding.attempt.role, EnvironmentRole::Builder);
    assert_eq!(binding.inputs, serde_json::to_value(&job.inputs).unwrap());
    assert!(!binding.validates(&wrong));
    assert!(!capture_ready(&runtime.store, &binding));
}

#[derive(Default)]
struct Fake {
    allocations: AtomicUsize,
    executions: AtomicUsize,
    images: AtomicUsize,
    image_response: Mutex<Option<Value>>,
    lost_create: AtomicBool,
    lost_create_all: AtomicBool,
    payment_required: AtomicBool,
    refused_creates: AtomicUsize,
    lost_command: AtomicBool,
    lost_delete: AtomicBool,
    gap: AtomicBool,
    foreign_account: AtomicBool,
    unknown: AtomicBool,
    stopped: AtomicBool,
    deleted: AtomicBool,
    create: Mutex<Option<Value>>,
    command: Mutex<Option<Value>>,
}
fn computer(fake: &Fake) -> Value {
    let deleted = fake.deleted.load(Ordering::SeqCst);
    let stopped = fake.stopped.load(Ordering::SeqCst);
    json!({"id":ID,"kind":"cloud","name":"Fixture","size":"small","state":if deleted{"deleted"}else if stopped{"stopped"}else{"awake"},"desired":if deleted{"deleted"}else if stopped{"stopped"}else{"running"},"online":!deleted&&!stopped})
}
fn command(fake: &Fake) -> Value {
    let unknown = fake.unknown.load(Ordering::SeqCst);
    json!({"id":COMMAND,"computer":ID,"state":if unknown{"unknown"}else{"succeeded"},"exit_code":if unknown{None}else{Some(0)},"digest":"a".repeat(64),"cancel_requested":false,"output_limit_bytes":262144,"output_expires_unix":2000000000,"stdout_truncated":false,"stderr_truncated":false})
}
async fn serve(State(fake): State<Arc<Fake>>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    let method = request.method().to_string();
    let query = request.uri().query().unwrap_or_default().to_owned();
    let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap_or_default();
    if path == "/v1/environment-computers/session" {
        return Json(json!({"account":{"id":if fake.foreign_account.load(Ordering::SeqCst){"acct_other"}else{"acct_fixture"},"label":"Test"},"workspaces":[]})).into_response();
    }
    if path.ends_with("/credentials-check") {
        return Json(json!({"ready":true})).into_response();
    }
    if path.ends_with("/computer") {
        return Json(json!(fake.create.lock().unwrap().is_some().then_some(ID))).into_response();
    }
    if path.ends_with("/command") {
        let id = fake.command.lock().unwrap().is_some().then_some(COMMAND);
        return Json(json!(id)).into_response();
    }
    if path.ends_with("/image") {
        fake.images.fetch_add(1, Ordering::SeqCst);
        if let Some(value) = fake.image_response.lock().unwrap().clone() {
            return Json(value).into_response();
        }
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    if path == "/v1/computers" && method == "POST" {
        if fake.payment_required.load(Ordering::SeqCst) {
            fake.refused_creates.fetch_add(1, Ordering::SeqCst);
            return (
                axum::http::StatusCode::PAYMENT_REQUIRED,
                [("payment-required", "private-payment-fixture")],
                Json(json!({"error":{"code":"payment_required"}})),
            )
                .into_response();
        }
        let mut first = fake.create.lock().unwrap();
        if let Some(saved) = first.as_ref() {
            assert_eq!(saved, &body)
        } else {
            *first = Some(body);
            fake.allocations.fetch_add(1, Ordering::SeqCst);
        }
        if fake.lost_create.swap(false, Ordering::SeqCst)
            || fake.lost_create_all.load(Ordering::SeqCst)
        {
            return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        return Json(computer(&fake)).into_response();
    }
    if path.ends_with("/commands") && method == "POST" {
        let mut first = fake.command.lock().unwrap();
        if let Some(saved) = first.as_ref() {
            assert_eq!(saved, &body)
        } else {
            *first = Some(body);
            fake.executions.fetch_add(1, Ordering::SeqCst);
        }
        if fake.lost_command.swap(false, Ordering::SeqCst) {
            return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        return Json(command(&fake)).into_response();
    }
    if path.ends_with("/output") {
        let params: BTreeMap<_, _> = url_params(&query);
        let start = params
            .get("stdout")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let bytes = b"build complete\n";
        return Json(json!({"command":command(&fake),"encoding":"base64","stdout":STANDARD.encode(&bytes[start..]),"stderr":"","next_cursor":{"stdout":bytes.len(),"stderr":0},"gaps":{"stdout":fake.gap.load(Ordering::SeqCst),"stderr":false},"terminal":true,"has_more":false})).into_response();
    }
    if path.ends_with("/stop") {
        fake.stopped.store(true, Ordering::SeqCst)
    }
    if path.ends_with("/wake") {
        fake.stopped.store(false, Ordering::SeqCst)
    }
    if path == format!("/v1/computers/{ID}") && method == "DELETE" {
        fake.deleted.store(true, Ordering::SeqCst);
        if fake.lost_delete.swap(false, Ordering::SeqCst) {
            return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        return axum::http::StatusCode::NO_CONTENT.into_response();
    }
    if path == format!("/v1/computers/{ID}") || path.ends_with("/stop") || path.ends_with("/wake") {
        return Json(computer(&fake)).into_response();
    }
    axum::http::StatusCode::NOT_FOUND.into_response()
}
fn url_params(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter_map(|s| s.split_once('='))
        .map(|(k, v)| (k.into(), v.into()))
        .collect()
}
struct Fixture {
    dir: tempfile::TempDir,
    c: Computer,
    fake: Arc<Fake>,
    origin: String,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path();
        let source = coder_environment::SourcePin {
            repository: Some("fixture/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        };
        let project = coder_environment::ProjectLink {
            workspace: "workspace".into(),
            project: "project".into(),
        };
        let env = coder_environment::Environment::new(
            "envc-fixture",
            project.clone(),
            source.clone(),
            crate::tests::recipe(),
            crate::now_ms(),
        )
        .unwrap();
        let layout = Layout::under(state);
        coder_environment::store::Store::under(layout.environments())
            .create(&env)
            .unwrap();
        let c = Computer::for_setup(
            Spec {
                id: "environment-computer".into(),
                chat: "setup-fixture".into(),
                owner: Principal {
                    workspace: "workspace".into(),
                    principal: "principal".into(),
                },
                project,
                source,
                base: None,
                size: "small".into(),
                credential_names: BTreeSet::new(),
                services: vec![],
                bounds: Bounds {
                    idle_ms: 60000,
                    observed_extension_ms: 60000,
                    absolute_ms: 3600000,
                },
            },
            "envc-fixture",
            crate::now_ms(),
        )
        .unwrap();
        coder_working_computer::store::Store::under(layout.computers())
            .create(&c)
            .unwrap();
        write(
            &state.join("environment-studio/envc-fixture/meta.json"),
            &json!({"account":"acct_fixture","public_computers":true}),
        )
        .unwrap();
        Store::under(state)
            .save_grant(
                "envc-fixture",
                &Grant {
                    account: "acct_fixture".into(),
                    workspace: None,
                    repository: "fixture/repo".into(),
                    private: false,
                    expires_unix: 2000000000,
                    credential: Value::Null,
                },
            )
            .unwrap();
        let fake = Arc::new(Fake::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new().fallback(serve).with_state(fake.clone());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            dir,
            c,
            fake,
            origin,
            server,
        }
    }
    fn provider(&self) -> Selected<Arc<coder_working_computer::provider::fake::FakeProvider>> {
        Selected {
            shared: true,
            legacy: Arc::new(coder_working_computer::provider::fake::FakeProvider::new(
                BTreeMap::new(),
                true,
            )),
            runtime: Some(Arc::new(
                Runtime::new(self.dir.path(), &self.origin, [7; 32]).unwrap(),
            )),
            store: Store::under(self.dir.path()),
        }
    }
    fn spec(&self) -> CommandSpec {
        CommandSpec {
            id: "build-command".into(),
            command: "cargo test --locked".into(),
            cwd: ".".into(),
            credential_names: BTreeSet::new(),
            env: BTreeMap::new(),
            timeout_seconds: 60,
            digest: "c".repeat(64),
        }
    }
}
#[tokio::test]
async fn a_payment_challenge_is_refused_without_allocation_or_an_automatic_retry() {
    let f = Fixture::new().await;
    f.fake.payment_required.store(true, Ordering::SeqCst);
    let result = f.provider().create(&f.c, "create").await;
    assert!(
        matches!(&result, Outcome::Failed { reason } if !reason.contains("private-payment-fixture"))
    );
    assert_eq!(f.fake.refused_creates.load(Ordering::SeqCst), 1);
    assert_eq!(f.fake.allocations.load(Ordering::SeqCst), 0);
    assert_eq!(f.fake.executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn lost_replies_and_restart_keep_the_same_allocation_command_and_cleanup() {
    let f = Fixture::new().await;
    f.fake.lost_create.store(true, Ordering::SeqCst);
    f.fake.lost_command.store(true, Ordering::SeqCst);
    f.fake.lost_delete.store(true, Ordering::SeqCst);
    let p = f.provider();
    assert!(matches!(p.create(&f.c,"create-once").await,Outcome::Done{value} if value==ID));
    assert!(matches!(
        p.start_command(&f.c, ID, &f.spec()).await,
        Outcome::Done { .. }
    ));
    drop(p);
    let p = f.provider();
    assert!(matches!(
        p.create(&f.c, "create-once").await,
        Outcome::Done { .. }
    ));
    assert!(matches!(
        p.start_command(&f.c, ID, &f.spec()).await,
        Outcome::Done { .. }
    ));
    let mut cursor = CommandCursor::default();
    let mut bytes = Vec::new();
    loop {
        let Outcome::Done { value } = p.read_command(&f.c, ID, &f.spec().id, cursor, 3).await
        else {
            panic!("output read failed")
        };
        cursor.stdout += value.stdout.len() as u64;
        bytes.extend(value.stdout);
        if matches!(value.progress, CommandProgress::Exited { code: 0 }) {
            break;
        }
        assert_eq!(value.progress, CommandProgress::Running);
    }
    assert_eq!(bytes, b"build complete\n");
    assert!(matches!(p.delete(&f.c, ID).await, Outcome::Done { .. }));
    assert!(p.store.binding(&f.c.id).unwrap().unwrap().deleted);
    assert_eq!(f.fake.allocations.load(Ordering::SeqCst), 1);
    assert_eq!(f.fake.executions.load(Ordering::SeqCst), 1);
    let saved = fs::read_to_string(p.store.path("attempts", &f.c.id).unwrap()).unwrap();
    assert!(!saved.contains("ecj_"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(p.store.path("attempts", &f.c.id).unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[tokio::test]
async fn an_accepted_command_with_a_lost_reference_is_observed_without_another_dispatch() {
    let f = Fixture::new().await;
    let p = f.provider();
    p.create(&f.c, "create").await;
    p.start_command(&f.c, ID, &f.spec()).await;
    let mut b = p.store.binding(&f.c.id).unwrap().unwrap();
    b.commands.get_mut(&f.spec().id).unwrap().reference = None;
    p.store.save(&b).unwrap();
    drop(p);
    let p = f.provider();
    let read = p
        .read_command(&f.c, ID, &f.spec().id, CommandCursor::default(), 65536)
        .await;
    assert!(matches!(read,Outcome::Done{value} if value.progress==CommandProgress::Exited{code:0}));
    assert_eq!(f.fake.executions.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn a_foreign_account_and_changed_source_never_allocate_another_computer() {
    let f = Fixture::new().await;
    f.fake.foreign_account.store(true, Ordering::SeqCst);
    let p = f.provider();
    assert!(matches!(
        p.create(&f.c, "create").await,
        Outcome::Unknown { .. }
    ));
    assert_eq!(f.fake.allocations.load(Ordering::SeqCst), 0);
    f.fake.foreign_account.store(false, Ordering::SeqCst);
    assert!(matches!(
        p.create(&f.c, "create").await,
        Outcome::Done { .. }
    ));
    let mut changed = f.c.clone();
    changed.source.revision = "e".repeat(40);
    assert!(matches!(
        p.create(&changed, "other").await,
        Outcome::Unknown { .. }
    ));
    assert_eq!(f.fake.allocations.load(Ordering::SeqCst), 1);
    let mut disabled = f.provider();
    disabled.runtime = None;
    assert!(matches!(
        disabled.delete(&f.c, ID).await,
        Outcome::Unknown { .. }
    ));
    assert!(!f.fake.deleted.load(Ordering::SeqCst));
}
#[tokio::test]
async fn a_private_checkpoint_is_not_a_reusable_image_and_output_gaps_never_pass() {
    let f = Fixture::new().await;
    let p = f.provider();
    p.create(&f.c, "create").await;
    let checkpoint = p.checkpoint(&f.c, ID, 1).await;
    assert!(matches!(checkpoint,Outcome::Done{value} if value.snapshot==ID));
    assert!(matches!(
        p.capture_image(&f.c, ID, "foreign-image").await,
        Outcome::Failed { .. }
    ));
    assert_eq!(f.fake.images.load(Ordering::SeqCst), 0);
    p.restore(&f.c, ID, None).await;
    p.start_command(&f.c, ID, &f.spec()).await;
    f.fake.gap.store(true, Ordering::SeqCst);
    assert!(matches!(
        p.read_command(&f.c, ID, &f.spec().id, CommandCursor::default(), 65536)
            .await,
        Outcome::Unknown { .. }
    ));
    f.fake.gap.store(false, Ordering::SeqCst);
    f.fake.unknown.store(true, Ordering::SeqCst);
    assert!(
        matches!(p.read_command(&f.c,ID,&f.spec().id,CommandCursor::default(),65536).await,Outcome::Done{value} if value.progress==CommandProgress::Lost)
    );
}

#[tokio::test]
async fn native_git_settings_and_subdirectories_keep_credentials_out_of_public_options() {
    let f = Fixture::new().await;
    let p = f.provider();
    p.create(&f.c, "create").await;
    let mut spec = f.spec();
    spec.cwd = "crates/example".into();
    spec.env.insert("GIT_CONFIG_COUNT".into(), "1".into());
    assert!(matches!(
        p.start_command(&f.c, ID, &spec).await,
        Outcome::Done { .. }
    ));
    let request = f.fake.command.lock().unwrap().clone().unwrap();
    assert_eq!(request["cwd"], ".");
    assert!(request["env"].is_null() || request["env"] == json!({}));
    let command = request["command"].as_str().unwrap();
    assert!(
        command.starts_with("cd 'crates/example' && env GIT_CONFIG_COUNT='1' sh -c "),
        "{command}"
    );
}

#[tokio::test]
async fn a_missing_shared_image_owner_never_falls_back_to_a_provider_default() {
    let f = Fixture::new().await;
    let p = f.provider();
    assert!(matches!(
        p.read_image("unindexed-output").await,
        Outcome::Unknown { .. }
    ));
    assert!(matches!(
        p.delete_image("unindexed-output").await,
        Outcome::Unknown { .. }
    ));
    assert_eq!(f.fake.images.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_expired_attempt_recovers_a_lost_create_reference_before_cleanup() {
    let f = Fixture::new().await;
    f.fake.lost_create_all.store(true, Ordering::SeqCst);
    let p = f.provider();
    assert!(matches!(p.create(&f.c, "once").await, Outcome::Done { .. }));
    assert_eq!(f.fake.allocations.load(Ordering::SeqCst), 1);
    let mut b = p.store.binding(&f.c.id).unwrap().unwrap();
    b.computer = None;
    p.store.save(&b).unwrap();
    let r = p.runtime.as_ref().unwrap();
    assert_eq!(
        r.sweep_expired(b.attempt.deadline_ms, &BTreeSet::new())
            .await,
        0
    );
    assert_eq!(
        r.sweep_expired(
            b.attempt.deadline_ms + crate::studio::leftovers::DEADLINE_GRACE_MS + 1,
            &BTreeSet::new()
        )
        .await,
        1
    );
    assert!(f.fake.deleted.load(Ordering::SeqCst));
    assert!(p.store.binding(&f.c.id).unwrap().unwrap().deleted);
    assert_eq!(f.fake.allocations.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_absent_image_does_not_finish_cleanup_while_a_capture_can_still_arrive() {
    let f = Fixture::new().await;
    let p = f.provider();
    p.create(&f.c, "create").await;
    p.store.bind_image("pending-output", &f.c.id).unwrap();
    *f.fake.image_response.lock().unwrap() = Some(json!({"kind":"done","value":false}));
    assert!(matches!(
        p.delete_image("pending-output").await,
        Outcome::Unknown { .. }
    ));
    assert!(!p.store.binding(&f.c.id).unwrap().unwrap().image_deleted);
}
