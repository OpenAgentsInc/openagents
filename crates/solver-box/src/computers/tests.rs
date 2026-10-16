use super::*;
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Request, State},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

const ID: &str = "cmp_0123456789abcdef0123";
const COMMAND: &str = "trn_0123456789abcdef0123";
const WORK: &str = "wrc0123456789abcdef01234567";

#[derive(Default)]
struct Fake {
    allocations: AtomicUsize,
    executions: AtomicUsize,
    deleted: AtomicBool,
    cleanup_calls: AtomicUsize,
    lose_create: AtomicBool,
    lose_execute: AtomicBool,
    lose_cleaned: AtomicBool,
    output_started: AtomicBool,
    hold_output: AtomicBool,
    gap: AtomicBool,
    command_failed: AtomicBool,
    already_done: AtomicBool,
    create: Mutex<Option<Value>>,
    command: Mutex<Option<Value>>,
}
fn computer(deleted: bool) -> Value {
    json!({"id":ID,"kind":"cloud","name":"Fixture","size":"large","state":if deleted{"deleted"}else{"awake"},"desired":if deleted{"deleted"}else{"running"},"online":!deleted})
}
fn command(failed: bool) -> Value {
    json!({"id":COMMAND,"computer":ID,"state":if failed{"failed"}else{"succeeded"},"exit_code":if failed{1}else{0},"digest":"a".repeat(64),"cancel_requested":false,"output_limit_bytes":262144,"output_expires_unix":2000000000,"stdout_truncated":false,"stderr_truncated":false})
}
async fn serve(State(fake): State<Arc<Fake>>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    let method = request.method().to_string();
    let bytes = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or_default();
    if path == "/v1/work/computer-session" {
        return Json(json!({"account":{"id":"acct_fixture","label":"Test"},"workspaces":[]}))
            .into_response();
    }
    if path.ends_with("/computer-credentials") {
        assert_eq!(body["computer"], ID);
        assert!(
            body["command"]["command"]
                .as_str()
                .unwrap()
                .contains("OA_SOURCE_REVISION")
        );
        assert_eq!(
            body["values"]["OA_SOLVER_GOOGLE_TOKEN"],
            "fixture-google-secret"
        );
        return Json(json!({"prepared":true})).into_response();
    }
    if path.ends_with("/computer-cleaned") {
        assert!(fake.deleted.load(Ordering::SeqCst));
        fake.cleanup_calls.fetch_add(1, Ordering::SeqCst);
        return if fake.lose_cleaned.swap(false, Ordering::SeqCst) {
            axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response()
        } else {
            Json(json!({"cleaned":true})).into_response()
        };
    }
    if path == "/v1/computers" && method == "POST" {
        let mut held = fake.create.lock().unwrap();
        if let Some(saved) = held.as_ref() {
            assert_eq!(&body, saved);
        } else {
            fake.allocations.fetch_add(1, Ordering::SeqCst);
            *held = Some(body);
        }
        return if fake.lose_create.swap(false, Ordering::SeqCst) {
            axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response()
        } else {
            Json(computer(false)).into_response()
        };
    }
    if path == format!("/v1/computers/{ID}") && method == "DELETE" {
        fake.deleted.store(true, Ordering::SeqCst);
        return axum::http::StatusCode::NO_CONTENT.into_response();
    }
    if path == format!("/v1/computers/{ID}") || path.ends_with("/wake") || path.ends_with("/stop") {
        return Json(computer(fake.deleted.load(Ordering::SeqCst))).into_response();
    }
    if path.ends_with("/commands") && method == "POST" {
        let mut held = fake.command.lock().unwrap();
        if let Some(saved) = held.as_ref() {
            assert_eq!(&body, saved);
        } else {
            fake.executions.fetch_add(1, Ordering::SeqCst);
            *held = Some(body);
        }
        return if fake.lose_execute.swap(false, Ordering::SeqCst) {
            axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response()
        } else {
            Json(command(false)).into_response()
        };
    }
    if path.ends_with("/output") {
        fake.output_started.store(true, Ordering::SeqCst);
        while fake.hold_output.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let stdout = if fake.already_done.load(Ordering::SeqCst) {
            format!(
                "{}\n",
                json!({"type":"solver_box_summary","started":true,"ok":false,"pr":null,"outcome":{"status":"already_done"}})
            )
        } else {
            format!(
                "{}\n",
                json!({"type":"solver_box_summary","ok":true,"pr":"https://github.com/fixture/repo/pull/1","cost_usd":0.01,"checks":[{"name":"fixture test","ok":true}]})
            )
        };
        return Json(json!({"command":command(fake.command_failed.load(Ordering::SeqCst)),"encoding":"base64","stdout":STANDARD.encode(stdout.as_bytes()),"stderr":"","next_cursor":{"stdout":stdout.len(),"stderr":0},"gaps":{"stdout":fake.gap.load(Ordering::SeqCst),"stderr":false},"terminal":true,"has_more":false})).into_response();
    }
    if path.contains("/commands/") {
        return Json(command(false)).into_response();
    }
    panic!("Unexpected fixture request: {method} {path}")
}
struct Server {
    site: Site,
    fake: Arc<Fake>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server() -> Server {
    let fake = Arc::new(Fake::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().fallback(serve).with_state(fake.clone());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    Server {
        site: Site::new(&base, "fixture-host", "fixture-host-secret").unwrap(),
        fake,
        task,
    }
}
fn job() -> Job {
    Job {
        account: "acct_fixture".into(),
        credential: "scj_fixture-job-secret".into(),
        cancelled: false,
        stop_reason: None,
        binding: json!({"create":{"size":"large","idempotency_key":format!("solver-{WORK}")},"environment":{"source":{"revision":"a".repeat(40)}},"payer":"acct_fixture","image":"fixture@sha256:1234"}),
    }
}
fn credentials() -> Credentials {
    Credentials {
        github_token: "fixture-github-secret".into(),
        openagents_token: None,
    }
}

#[tokio::test]
async fn lost_create_exec_and_cleanup_replies_keep_one_allocation_and_execution() {
    let server = server().await;
    let root = tempfile::tempdir().unwrap();
    server.fake.lose_create.store(true, Ordering::SeqCst);
    server.fake.lose_execute.store(true, Ordering::SeqCst);
    server.fake.lose_cleaned.store(true, Ordering::SeqCst);
    let mut run = Run::new("fixture/repo", 1);
    run.image = "fixture-image".into();
    let first = execute(
        root.path(),
        &server.site,
        WORK,
        &job(),
        &run,
        &credentials(),
        &GoogleToken::Fixture,
        &Stop::default(),
        &|_| {},
    )
    .await;
    assert!(first.is_err());
    assert!(server.fake.deleted.load(Ordering::SeqCst));
    let second = execute(
        root.path(),
        &server.site,
        WORK,
        &job(),
        &run,
        &credentials(),
        &GoogleToken::Fixture,
        &Stop::default(),
        &|_| {},
    )
    .await
    .unwrap();
    assert_eq!(second["ok"], true);
    assert!(second["cost_usd"].is_null());
    assert_eq!(server.fake.allocations.load(Ordering::SeqCst), 1);
    assert_eq!(server.fake.executions.load(Ordering::SeqCst), 1);
    assert_eq!(server.fake.cleanup_calls.load(Ordering::SeqCst), 2);
    let bytes = fs::read_to_string(root.path().join(format!("{WORK}.json"))).unwrap();
    for secret in [
        "fixture-github-secret",
        "fixture-google-secret",
        "scj_fixture-job-secret",
        "fixture-host-secret",
    ] {
        assert!(!bytes.contains(secret));
    }
}

#[tokio::test]
async fn a_restarted_dispatcher_attaches_to_its_saved_command() {
    let server = server().await;
    let root = tempfile::tempdir().unwrap();
    server.fake.hold_output.store(true, Ordering::SeqCst);
    let saved_root = root.path().to_path_buf();
    let site = server.site.clone();
    let task = tokio::spawn(async move {
        execute(
            &saved_root,
            &site,
            WORK,
            &job(),
            &Run::new("fixture/repo", 1),
            &credentials(),
            &GoogleToken::Fixture,
            &Stop::default(),
            &|_| {},
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !server.fake.output_started.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    server.fake.hold_output.store(false, Ordering::SeqCst);
    let result = execute(
        root.path(),
        &server.site,
        WORK,
        &job(),
        &Run::new("fixture/repo", 1),
        &credentials(),
        &GoogleToken::Fixture,
        &Stop::default(),
        &|_| {},
    )
    .await
    .unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(server.fake.allocations.load(Ordering::SeqCst), 1);
    assert_eq!(server.fake.executions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn missing_output_and_command_failure_never_become_success() {
    for gap in [true, false] {
        let server = server().await;
        let root = tempfile::tempdir().unwrap();
        server.fake.gap.store(gap, Ordering::SeqCst);
        server.fake.command_failed.store(!gap, Ordering::SeqCst);
        let result = execute(
            root.path(),
            &server.site,
            WORK,
            &job(),
            &Run::new("fixture/repo", 1),
            &credentials(),
            &GoogleToken::Fixture,
            &Stop::default(),
            &|_| {},
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], false);
        assert!(server.fake.deleted.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn an_already_done_summary_keeps_the_existing_job_outcome() {
    let server = server().await;
    let root = tempfile::tempdir().unwrap();
    server.fake.already_done.store(true, Ordering::SeqCst);
    server.fake.command_failed.store(true, Ordering::SeqCst);
    let result = execute(
        root.path(),
        &server.site,
        WORK,
        &job(),
        &Run::new("fixture/repo", 1),
        &credentials(),
        &GoogleToken::Fixture,
        &Stop::default(),
        &|_| {},
    )
    .await
    .unwrap();
    assert_eq!(result["outcome"]["status"], "already_done");
    assert_eq!(result["ok"], false);
    assert!(server.fake.deleted.load(Ordering::SeqCst));
}

#[tokio::test]
async fn duplicate_delivery_is_locked_and_foreign_account_fails_before_allocation() {
    let root = tempfile::tempdir().unwrap();
    let lock = Journal::open(root.path(), WORK).unwrap();
    assert!(Journal::open(root.path(), WORK).is_err());
    drop(lock);
    assert!(Journal::open(root.path(), WORK).is_ok());
    let server = server().await;
    let mut job = job();
    job.account = "acct_other".into();
    assert!(
        execute(
            root.path(),
            &server.site,
            WORK,
            &job,
            &Run::new("fixture/repo", 1),
            &credentials(),
            &GoogleToken::Fixture,
            &Stop::default(),
            &|_| {}
        )
        .await
        .is_err()
    );
    assert_eq!(server.fake.allocations.load(Ordering::SeqCst), 0);
}

#[test]
fn the_container_keeps_the_selected_source_even_when_remote_main_advances() {
    use std::process::Command;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let tools = root.path().join("bin");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&tools).unwrap();
    let config = root.path().join("git-config");
    let git = |args: &[&str]| {
        let result = Command::new("git")
            .args(args)
            .current_dir(&source)
            .env("GIT_CONFIG_GLOBAL", &config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8_lossy(&result.stdout).trim().to_owned()
    };
    git(&["init", "-q", "-b", "main"]);
    fs::write(source.join("value"), "first").unwrap();
    git(&["add", "value"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.test",
        "commit",
        "-qm",
        "First",
    ]);
    let pin = git(&["rev-parse", "HEAD"]);
    fs::write(source.join("value"), "second").unwrap();
    git(&["add", "value"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.test",
        "commit",
        "-qm",
        "Second",
    ]);
    assert_ne!(pin, git(&["rev-parse", "HEAD"]));
    git(&[
        "config",
        "--global",
        &format!("url.{}.insteadOf", source.display()),
        "https://github.com/fixture/repo.git",
    ]);
    fs::write(
        tools.join("gh"),
        "#!/bin/sh\nif [ \"$1\" = issue ]; then printf 'OPEN\\n'; fi\n",
    )
    .unwrap();
    fs::write(tools.join("coder-new"),r#"#!/bin/sh
set -eu
repo="$OA_SOLVER_WORK/repo"
git -C "$repo" fetch -q origin main
[ "$(git -C "$repo" rev-parse origin/main)" = "$OA_SOURCE_REVISION" ]
[ "$(git -C "$repo" rev-parse main)" = "$OA_SOURCE_REVISION" ]
[ "$(git -C "$repo" config --get remote.origin.pushurl)" = 'https://github.com/fixture/repo.git' ]
mkdir -p "$OA_SOLVER_WORK/state/fixture"
printf '%s' '{"solver":{"pull_request":"https://github.com/fixture/repo/pull/1"},"checks":[{"name":"source pin","ok":true}]}' > "$OA_SOLVER_WORK/state/fixture/summary.json"
"#).unwrap();
    for tool in ["gh", "coder-new"] {
        fs::set_permissions(tools.join(tool), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let output = Command::new("bash")
        .arg("-c")
        .arg(include_str!("../../../../deploy/solver-box/solver-box-run"))
        .env("GIT_CONFIG_GLOBAL", &config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "PATH",
            format!("{}:{}", tools.display(), std::env::var("PATH").unwrap()),
        )
        .env("OA_REPO", "fixture/repo")
        .env("OA_ISSUE", "1")
        .env("GH_TOKEN", "fixture-github-secret")
        .env("OA_SOLVER_ENV", root.path().join("missing-env"))
        .env("OA_SOLVER_WORK", root.path().join("work"))
        .env("OA_SOURCE_REVISION", &pin)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: Value = serde_json::from_str(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(summary["ok"], true);
}
