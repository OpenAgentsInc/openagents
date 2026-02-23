use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use clap::Parser;
use openagents_runtime_service::bridge::{
    ProviderAdV1, ReceiptPointerV1, build_provider_ad_event, build_receipt_pointer_event,
    receipt_sha256_from_utf8, validate_phase0_bridge_event,
};
use openagents_runtime_service::config::{AuthorityWriteMode, Config as RuntimeConfig};
use openagents_runtime_service::marketplace::ProviderSelection;
use openagents_runtime_service::types::RuntimeRun;
use openagents_runtime_service::workers::WorkerSnapshot;
use protocol::hash::canonical_hash;
use protocol::jobs::JobRequest as _;
use protocol::jobs::sandbox::{
    CommandResult, EnvInfo, NetworkPolicy, ResourceLimits, SandboxConfig, SandboxStatus,
};
use protocol::jobs::{SandboxRunRequest, SandboxRunResponse};
use protocol::provenance::Provenance;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};
use uuid::Uuid;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    output_dir: PathBuf,
}

#[derive(Clone)]
struct ProviderCaps {
    max_timeout_secs: u32,
}

#[derive(Clone)]
struct ProviderState {
    id: String,
    enabled: Arc<AtomicBool>,
    caps: ProviderCaps,
    executions: Arc<AtomicU64>,
    cache_hits: Arc<AtomicU64>,
    cached: Arc<Mutex<HashMap<String, SandboxRunResponse>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProviderStatusResponse {
    id: String,
    enabled: bool,
    executions: u64,
    cache_hits: u64,
    max_timeout_secs: u32,
}

struct ProviderHandle {
    base_url: String,
    state: ProviderState,
    shutdown: oneshot::Sender<()>,
}

struct RuntimeHandle {
    base_url: String,
    shutdown: oneshot::Sender<()>,
}

#[derive(Clone)]
struct FixtureRepo {
    path: PathBuf,
    initial_commit: String,
}

#[derive(Debug, Deserialize)]
struct RunResponse {
    run: RuntimeRun,
}

#[derive(Debug, Deserialize)]
struct WorkerResponse {
    worker: WorkerSnapshot,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("create output dir {}", args.output_dir.display()))?;

    let client = reqwest::Client::new();

    let runtime = start_runtime().await?;
    wait_for_http_ok(
        &client,
        &runtime.base_url,
        "/healthz",
        Duration::from_secs(3),
    )
    .await?;

    // Provider A is the primary provider (used for happy path and offline tests).
    let provider_a = start_provider(
        "provider-a",
        ProviderCaps {
            max_timeout_secs: 120,
        },
    )
    .await?;
    wait_for_http_ok(
        &client,
        &provider_a.base_url,
        "/healthz",
        Duration::from_secs(3),
    )
    .await?;
    // Provider B is the reserve provider (used for fallback in offline test).
    let provider_b = start_provider(
        "provider-b",
        ProviderCaps {
            max_timeout_secs: 120,
        },
    )
    .await?;
    wait_for_http_ok(
        &client,
        &provider_b.base_url,
        "/healthz",
        Duration::from_secs(3),
    )
    .await?;

    let fixture_root = args.output_dir.join("fixture_repo");
    let fixture = create_fixture_repo(&fixture_root)?;

    // Phase 0: enroll devices/providers (Zone 0.5) with caps.
    let owner_user_id = 1_u64;
    enroll_provider(
        &client,
        &runtime.base_url,
        owner_user_id,
        "device:vignette-provider-a",
        &provider_a,
        false,
        1000,
    )
    .await?;
    enroll_provider(
        &client,
        &runtime.base_url,
        999,
        "device:vignette-provider-b",
        &provider_b,
        true,
        1500,
    )
    .await?;

    // 1) Happy path: issue -> verified patch -> pay-after-verify -> bundle.
    let happy_dir = args.output_dir.join("happy");
    std::fs::create_dir_all(&happy_dir)?;
    run_happy_path(
        &client,
        &runtime.base_url,
        owner_user_id,
        &fixture,
        &provider_a,
        &happy_dir,
    )
    .await?;

    // 2) Verification fails: no payment release, provider penalized/quarantined.
    let verify_fail_dir = args.output_dir.join("verification_fail");
    std::fs::create_dir_all(&verify_fail_dir)?;
    run_verification_fail(
        &client,
        &runtime.base_url,
        owner_user_id,
        &fixture,
        &provider_a,
        &verify_fail_dir,
    )
    .await?;

    // 3) Provider offline mid-run: quarantine + fallback to reserve provider.
    let offline_dir = args.output_dir.join("provider_offline_mid_run");
    std::fs::create_dir_all(&offline_dir)?;
    run_provider_offline_mid_run(
        &client,
        &runtime.base_url,
        owner_user_id,
        &fixture,
        &provider_a,
        &provider_b,
        &offline_dir,
    )
    .await?;

    // 4) Emergency disable: blocks new work immediately.
    let emergency_dir = args.output_dir.join("emergency_disable");
    std::fs::create_dir_all(&emergency_dir)?;
    run_emergency_disable(
        &client,
        &runtime.base_url,
        owner_user_id,
        &fixture,
        &provider_a,
        &emergency_dir,
    )
    .await?;

    // Shutdown servers.
    let _ = provider_a.shutdown.send(());
    let _ = provider_b.shutdown.send(());
    let _ = runtime.shutdown.send(());

    Ok(())
}

async fn start_runtime() -> Result<RuntimeHandle> {
    let mut config = RuntimeConfig::from_env().context("load runtime config")?;
    config.service_name = "runtime-vignette".to_string();
    config.build_sha = "vignette".to_string();
    config.authority_write_mode = AuthorityWriteMode::RustActive;
    config.bind_addr = "127.0.0.1:0".parse().context("parse bind addr")?;

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .context("bind runtime listener")?;
    let addr = listener.local_addr().context("runtime local_addr")?;
    let app = openagents_runtime_service::build_app(config);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(err) = server.await {
            tracing::error!(error = %err, "runtime server failed");
        }
    });

    Ok(RuntimeHandle {
        base_url: format!("http://{addr}"),
        shutdown: shutdown_tx,
    })
}

async fn start_provider(id: &str, caps: ProviderCaps) -> Result<ProviderHandle> {
    let state = ProviderState {
        id: id.to_string(),
        enabled: Arc::new(AtomicBool::new(true)),
        caps,
        executions: Arc::new(AtomicU64::new(0)),
        cache_hits: Arc::new(AtomicU64::new(0)),
        cached: Arc::new(Mutex::new(HashMap::new())),
    };

    let router = Router::new()
        .route("/healthz", get(provider_health))
        .route("/v1/status", get(provider_status))
        .route("/v1/disable", post(provider_disable))
        .route("/v1/enable", post(provider_enable))
        .route("/v1/sandbox_run", post(provider_sandbox_run))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind provider listener")?;
    let addr = listener.local_addr().context("provider local_addr")?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(err) = server.await {
            tracing::error!(error = %err, "provider server failed");
        }
    });

    Ok(ProviderHandle {
        base_url: format!("http://{addr}"),
        state,
        shutdown: shutdown_tx,
    })
}

async fn wait_for_http_ok(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    let url = format!("{base_url}{path}");
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow!("timeout waiting for {}", url));
        }
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

async fn provider_health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn provider_status(State(state): State<ProviderState>) -> Json<ProviderStatusResponse> {
    Json(ProviderStatusResponse {
        id: state.id.clone(),
        enabled: state.enabled.load(Ordering::Relaxed),
        executions: state.executions.load(Ordering::Relaxed),
        cache_hits: state.cache_hits.load(Ordering::Relaxed),
        max_timeout_secs: state.caps.max_timeout_secs,
    })
}

async fn provider_disable(State(state): State<ProviderState>) -> impl IntoResponse {
    state.enabled.store(false, Ordering::Relaxed);
    (StatusCode::OK, Json(json!({"ok": true, "enabled": false})))
}

async fn provider_enable(State(state): State<ProviderState>) -> impl IntoResponse {
    state.enabled.store(true, Ordering::Relaxed);
    (StatusCode::OK, Json(json!({"ok": true, "enabled": true})))
}

async fn provider_sandbox_run(
    headers: HeaderMap,
    State(state): State<ProviderState>,
    Json(request): Json<SandboxRunRequest>,
) -> Result<Json<SandboxRunResponse>, (StatusCode, String)> {
    if !state.enabled.load(Ordering::Relaxed) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "provider disabled".to_string(),
        ));
    }

    let idempotency_key = header_string(&headers, "x-idempotency-key");
    if let Some(key) = idempotency_key.as_deref() {
        let cached = state.cached.lock().await;
        if let Some(existing) = cached.get(key) {
            state.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Json(existing.clone()));
        }
    }

    state.executions.fetch_add(1, Ordering::Relaxed);

    let started_at = Utc::now();
    let response = execute_sandbox_request(&state, &request)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let duration_ms = (Utc::now() - started_at).num_milliseconds().max(0) as u64;

    let mut response = response;
    response.provenance = response
        .provenance
        .clone()
        .with_duration(duration_ms)
        .with_executed_at(Utc::now().timestamp() as u64);

    if let Some(key) = idempotency_key {
        let mut cached = state.cached.lock().await;
        cached.insert(key, response.clone());
    }

    Ok(Json(response))
}

fn header_string(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

async fn execute_sandbox_request(
    state: &ProviderState,
    request: &SandboxRunRequest,
) -> Result<SandboxRunResponse> {
    let repo_source = request.repo.source.trim();
    if repo_source.is_empty() {
        return Err(anyhow!("repo.source is required"));
    }

    let temp_root = std::env::temp_dir()
        .join("openagents-vignette")
        .join("provider")
        .join(Uuid::now_v7().to_string());
    std::fs::create_dir_all(&temp_root)
        .with_context(|| format!("create temp dir {}", temp_root.display()))?;
    let repo_dir = temp_root.join("repo");

    let mut clone_cmd = Command::new("git");
    clone_cmd.args([
        "clone",
        "--quiet",
        repo_source,
        repo_dir.to_string_lossy().as_ref(),
    ]);
    clone_cmd.stdin(Stdio::null());
    run_checked(clone_cmd, "git clone fixture repo")?;

    if let Some(git_ref) = request.repo.git_ref.as_deref()
        && !git_ref.trim().is_empty()
    {
        let mut checkout_cmd = Command::new("git");
        checkout_cmd.current_dir(&repo_dir);
        checkout_cmd.args(["checkout", "--quiet", git_ref]);
        checkout_cmd.stdin(Stdio::null());
        run_checked(checkout_cmd, "git checkout ref")?;
    }

    let timeout_secs = request
        .sandbox
        .resources
        .timeout_secs
        .min(state.caps.max_timeout_secs)
        .max(1);

    let mut runs = Vec::new();
    let mut overall_status = SandboxStatus::Success;
    let mut error = None;

    for cmd in &request.commands {
        if !state.enabled.load(Ordering::Relaxed) {
            overall_status = SandboxStatus::Cancelled;
            error = Some("provider disabled mid-run".to_string());
            break;
        }

        let started = std::time::Instant::now();
        let output = run_shell_command(
            cmd.cmd.as_str(),
            &repo_dir,
            &request.env,
            Duration::from_secs(timeout_secs.into()),
        )
        .await;
        let duration_ms = started.elapsed().as_millis() as u64;

        match output {
            Ok(out) => {
                let stdout_hash = sha256_prefixed_bytes(&out.stdout);
                let stderr_hash = sha256_prefixed_bytes(&out.stderr);
                let exit_code = out.status.code().unwrap_or(1);

                runs.push(CommandResult {
                    cmd: cmd.cmd.clone(),
                    exit_code,
                    duration_ms,
                    stdout_sha256: stdout_hash,
                    stderr_sha256: stderr_hash,
                    stdout_preview: preview_bytes(&out.stdout),
                    stderr_preview: preview_bytes(&out.stderr),
                });

                if exit_code != 0 {
                    overall_status = SandboxStatus::Failed;
                    if !cmd.continue_on_fail {
                        break;
                    }
                }
            }
            Err(err) => {
                overall_status = SandboxStatus::Error;
                error = Some(err.to_string());
                runs.push(CommandResult {
                    cmd: cmd.cmd.clone(),
                    exit_code: 1,
                    duration_ms,
                    stdout_sha256: sha256_prefixed(""),
                    stderr_sha256: sha256_prefixed(""),
                    stdout_preview: None,
                    stderr_preview: Some(truncate_string(err.to_string(), 240)),
                });
                break;
            }
        }
    }

    let env_info = EnvInfo {
        image_digest: request.sandbox.image_digest.clone(),
        hostname: std::env::var("HOSTNAME").ok(),
        system_info: Some(system_info()?),
    };

    let request_hash = request
        .compute_hash()
        .unwrap_or_else(|_| "sha256:invalid".to_string());
    let output_hash = canonical_hash(&json!({"runs": runs, "status": overall_status})).ok();
    let provenance = Provenance::new("sandbox-local")
        .with_input_hash(request_hash)
        .with_output_hash(output_hash.unwrap_or_else(|| "sha256:invalid".to_string()));

    Ok(SandboxRunResponse {
        env_info,
        runs,
        artifacts: Vec::new(),
        status: overall_status,
        error,
        provenance,
    })
}

fn system_info() -> Result<String> {
    let output = Command::new("uname")
        .arg("-a")
        .output()
        .context("uname -a")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_checked(mut cmd: Command, what: &str) -> Result<String> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to run: {what}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "{what} failed (exit={:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn run_shell_command(
    command: &str,
    cwd: &Path,
    env: &HashMap<String, String>,
    timeout: Duration,
) -> Result<std::process::Output> {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd.current_dir(cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let child = cmd.spawn().context("spawn shell command")?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| anyhow!("command timed out after {}s", timeout.as_secs()))?
        .context("collect command output")?;
    Ok(output)
}

fn sha256_prefixed_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", hex::encode(digest))
}

fn sha256_prefixed(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

fn preview_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    Some(truncate_string(text.to_string(), 240))
}

fn truncate_string(mut text: String, max_len: usize) -> String {
    if text.len() <= max_len {
        return text;
    }
    text.truncate(max_len);
    text.push_str("... [truncated]");
    text
}

fn create_fixture_repo(path: &Path) -> Result<FixtureRepo> {
    if path.exists() {
        std::fs::remove_dir_all(path)
            .with_context(|| format!("remove existing fixture {}", path.display()))?;
    }
    std::fs::create_dir_all(path.join("src"))?;
    std::fs::create_dir_all(path.join(".openagents"))?;

    std::fs::write(
        path.join("Cargo.toml"),
        r#"[package]
name = "vignette_repo"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )?;
    std::fs::write(
        path.join("src/lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 {
    // BUG: should be addition.
    a - b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_works() {
        assert_eq!(add(2, 2), 4);
    }
}
"#,
    )?;
    std::fs::write(
        path.join(".openagents/issues.json"),
        serde_json::to_string_pretty(&json!([
          {
            "number": 1,
            "title": "Fix add() math bug",
            "description": "add() currently subtracts; fix to addition and make tests pass.",
            "status": "open",
            "priority": "high",
            "is_blocked": false
          }
        ]))?,
    )?;

    let mut init_cmd = Command::new("git");
    init_cmd.current_dir(path);
    init_cmd.args(["init", "--quiet"]);
    run_checked(init_cmd, "git init")?;

    let mut email_cmd = Command::new("git");
    email_cmd.current_dir(path);
    email_cmd.args(["config", "user.email", "vignette@openagents.local"]);
    run_checked(email_cmd, "git config email")?;

    let mut name_cmd = Command::new("git");
    name_cmd.current_dir(path);
    name_cmd.args(["config", "user.name", "OpenAgents Vignette"]);
    run_checked(name_cmd, "git config name")?;

    let mut add_cmd = Command::new("git");
    add_cmd.current_dir(path);
    add_cmd.args(["add", "."]);
    run_checked(add_cmd, "git add")?;

    let mut commit_cmd = Command::new("git");
    commit_cmd.current_dir(path);
    commit_cmd.args(["commit", "-m", "fixture: initial repo", "--quiet"]);
    run_checked(commit_cmd, "git commit")?;

    let mut rev_cmd = Command::new("git");
    rev_cmd.current_dir(path);
    rev_cmd.args(["rev-parse", "HEAD"]);
    let initial_commit = run_checked(rev_cmd, "git rev-parse HEAD")?;

    Ok(FixtureRepo {
        path: path.to_path_buf(),
        initial_commit,
    })
}

fn clone_fixture_repo(template: &FixtureRepo, dest: &Path) -> Result<FixtureRepo> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)
            .with_context(|| format!("remove existing clone {}", dest.display()))?;
    }
    std::fs::create_dir_all(dest.parent().unwrap_or_else(|| Path::new(".")))?;

    let mut clone_cmd = Command::new("git");
    clone_cmd.args([
        "clone",
        "--quiet",
        template.path.to_string_lossy().as_ref(),
        dest.to_string_lossy().as_ref(),
    ]);
    run_checked(clone_cmd, "git clone scenario repo")?;

    let mut email_cmd = Command::new("git");
    email_cmd.current_dir(dest);
    email_cmd.args(["config", "user.email", "vignette@openagents.local"]);
    run_checked(email_cmd, "git config email (clone)")?;

    let mut name_cmd = Command::new("git");
    name_cmd.current_dir(dest);
    name_cmd.args(["config", "user.name", "OpenAgents Vignette"]);
    run_checked(name_cmd, "git config name (clone)")?;

    Ok(FixtureRepo {
        path: dest.to_path_buf(),
        initial_commit: template.initial_commit.clone(),
    })
}

fn apply_issue_fix(repo: &FixtureRepo) -> Result<(String, String)> {
    let branch = "autopilot/issue-1".to_string();
    let mut checkout_cmd = Command::new("git");
    checkout_cmd.current_dir(&repo.path);
    checkout_cmd.args(["checkout", "-b", &branch, "--quiet"]);
    run_checked(checkout_cmd, "git checkout -b")?;

    let lib_path = repo.path.join("src/lib.rs");
    let content = std::fs::read_to_string(&lib_path).context("read lib.rs")?;
    let updated = content.replace("a - b", "a + b");
    if updated == content {
        return Err(anyhow!(
            "fixture lib.rs did not contain expected bug string"
        ));
    }
    std::fs::write(&lib_path, updated).context("write lib.rs")?;

    let mut add_cmd = Command::new("git");
    add_cmd.current_dir(&repo.path);
    add_cmd.args(["add", "src/lib.rs"]);
    run_checked(add_cmd, "git add fix")?;

    let mut commit_cmd = Command::new("git");
    commit_cmd.current_dir(&repo.path);
    commit_cmd.args(["commit", "-m", "fix: add uses addition", "--quiet"]);
    run_checked(commit_cmd, "git commit fix")?;

    let mut rev_cmd = Command::new("git");
    rev_cmd.current_dir(&repo.path);
    rev_cmd.args(["rev-parse", "HEAD"]);
    let commit = run_checked(rev_cmd, "git rev-parse HEAD")?;

    Ok((branch, commit))
}

async fn enroll_provider(
    client: &reqwest::Client,
    runtime_base: &str,
    owner_user_id: u64,
    worker_id: &str,
    provider: &ProviderHandle,
    reserve_pool: bool,
    min_price_msats: u64,
) -> Result<WorkerSnapshot> {
    let url = format!("{runtime_base}/internal/v1/workers");
    let body = json!({
        "worker_id": worker_id,
        "owner_user_id": owner_user_id,
        "adapter": "openagents_compute_provider",
        "metadata": {
            "roles": ["client", "provider"],
            "provider_id": provider.state.id,
            "provider_base_url": provider.base_url,
            "capabilities": ["oa.sandbox_run.v1"],
            "min_price_msats": min_price_msats,
            "reserve_pool": reserve_pool,
            "caps": {
                "max_timeout_secs": provider.state.caps.max_timeout_secs
            }
        }
    });
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .context("register worker")?;
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("worker registration failed: {}", text));
    }
    let parsed: WorkerResponse = resp.json().await.context("parse worker response")?;
    Ok(parsed.worker)
}

async fn run_happy_path(
    client: &reqwest::Client,
    runtime_base: &str,
    owner_user_id: u64,
    fixture: &FixtureRepo,
    provider: &ProviderHandle,
    out_dir: &Path,
) -> Result<()> {
    let repo = clone_fixture_repo(fixture, &out_dir.join("repo"))?;
    let (branch, commit) = apply_issue_fix(&repo)?;
    let run = start_run(
        client,
        runtime_base,
        "autopilot-desktop:vignette",
        json!({
            "policy_bundle_id": "vignette.phase0",
            "repo_path": repo.path,
            "issue_number": 1,
            "branch": branch,
            "commit": commit,
        }),
    )
    .await?;

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "DeviceEnrolled",
        json!({"device_id": "device:vignette-provider-a", "roles": ["client", "provider"]}),
        Some("device-enrolled".to_string()),
    )
    .await?;
    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "CapsSet",
        json!({"device_id": "device:vignette-provider-a", "caps": {"max_timeout_secs": provider.state.caps.max_timeout_secs}}),
        Some("caps-set".to_string()),
    )
    .await?;
    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "IssueClaimed",
        json!({"issue_number": 1}),
        Some("issue-claimed".to_string()),
    )
    .await?;
    append_event(
        client,
        runtime_base,
        run.id,
        "run.started",
        json!({"ok": true}),
        Some("run-started".to_string()),
    )
    .await?;

    let request = sandbox_request(
        &repo.path,
        Some(commit.clone()),
        vec!["cargo test".to_string()],
    );
    let job_hash = request.compute_hash().context("compute sandbox job hash")?;

    let dispatch =
        dispatch_sandbox_via_runtime(client, runtime_base, owner_user_id, &request).await?;
    if dispatch.job_hash != job_hash {
        return Err(anyhow!(
            "runtime dispatch job_hash mismatch: expected {}, got {}",
            job_hash,
            dispatch.job_hash
        ));
    }
    let selection = dispatch.selection;
    if selection.provider.provider_id != provider.state.id {
        return Err(anyhow!(
            "unexpected dispatched provider: expected {}, got {}",
            provider.state.id,
            selection.provider.provider_id
        ));
    }
    let response = dispatch.response;
    let latency_ms = dispatch.latency_ms;

    // Retry dispatch: same idempotency key must not re-execute provider-side.
    let dispatch_retry =
        dispatch_sandbox_via_runtime(client, runtime_base, owner_user_id, &request).await?;
    assert_same_sandbox_response(&response, &dispatch_retry.response)?;
    assert_provider_idempotency(client, provider, 1, 1).await?;

    let job_dispatched_receipt = json!({
        "job_type": SandboxRunRequest::JOB_TYPE,
        "job_hash": job_hash,
        "provider_id": selection.provider.provider_id,
        "idempotency_key": job_hash
    });
    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "JobDispatched",
        job_dispatched_receipt.clone(),
        Some(format!("job-dispatched:{job_hash}")),
    )
    .await?;
    assert_runtime_idempotent_key(
        client,
        runtime_base,
        run.id,
        "receipt",
        json!({"receipt_type": "JobDispatched", "payload": job_dispatched_receipt}),
        format!("job-dispatched:{job_hash}"),
    )
    .await?;
    append_tool_event(
        client,
        runtime_base,
        run.id,
        "oa.sandbox_run.v1",
        &request,
        &response,
        latency_ms,
    )
    .await?;
    let job_completed_receipt =
        json!({"job_hash": job_hash, "status": format!("{:?}", response.status).to_lowercase()});
    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "JobCompleted",
        job_completed_receipt.clone(),
        Some(format!("job-completed:{job_hash}")),
    )
    .await?;
    assert_runtime_idempotent_key(
        client,
        runtime_base,
        run.id,
        "receipt",
        json!({"receipt_type": "JobCompleted", "payload": job_completed_receipt}),
        format!("job-completed:{job_hash}"),
    )
    .await?;

    let settle_before = fetch_run(client, runtime_base, run.id).await?.events.len();
    let settlement = settle_sandbox_with_runtime(
        client,
        runtime_base,
        owner_user_id,
        run.id,
        &selection,
        &request,
        &response,
    )
    .await?;
    let expected_amount_msats = selection.provider.min_price_msats.unwrap_or(1_000);
    if settlement.job_hash != job_hash {
        return Err(anyhow!(
            "runtime settlement job_hash mismatch: expected {}, got {}",
            job_hash,
            settlement.job_hash
        ));
    }
    if settlement.amount_msats != expected_amount_msats {
        return Err(anyhow!(
            "runtime settlement amount mismatch: expected {}, got {}",
            expected_amount_msats,
            settlement.amount_msats
        ));
    }
    if !settlement.reservation_id.starts_with("rsv_") {
        return Err(anyhow!(
            "runtime settlement reservation_id missing rsv_ prefix: {}",
            settlement.reservation_id
        ));
    }
    if !settlement.verification_passed || settlement.settlement_status != "released" {
        return Err(anyhow!(
            "expected verification pass + settlement release, got passed={} status={}",
            settlement.verification_passed,
            settlement.settlement_status
        ));
    }
    if settlement.exit_code != 0 {
        return Err(anyhow!(
            "expected exit_code=0 for happy path, got {}",
            settlement.exit_code
        ));
    }
    if !settlement.violations.is_empty() {
        return Err(anyhow!(
            "happy path unexpectedly contained provider violations: {:?}",
            settlement.violations
        ));
    }

    // Settlement must be idempotent (no double-spend, no duplicate receipts/events).
    let _settlement_retry = settle_sandbox_with_runtime(
        client,
        runtime_base,
        owner_user_id,
        run.id,
        &selection,
        &request,
        &response,
    )
    .await?;
    let settle_after = fetch_run(client, runtime_base, run.id).await?.events.len();
    if settle_after != settle_before + 5 {
        return Err(anyhow!(
            "unexpected settlement event count delta: before={} after={}",
            settle_before,
            settle_after
        ));
    }

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "ForgeUpdated",
        json!({"branch": branch, "commit": commit}),
        Some("forge-updated".to_string()),
    )
    .await?;

    append_event(
        client,
        runtime_base,
        run.id,
        "run.finished",
        json!({"status": "succeeded"}),
        Some("run-finished".to_string()),
    )
    .await?;

    write_bundle(client, runtime_base, run.id, out_dir, &format!(
        "# PR Summary (Vignette Phase 0)\n\n- Issue: #1 Fix add() math bug\n- Branch: {branch}\n- Commit: {commit}\n- Verification: `cargo test`\n"
    ))
    .await?;

    write_bridge_events(
        out_dir,
        runtime_base,
        run.id,
        &provider.state.id,
        &job_hash,
        "released",
    )?;
    write_metrics(out_dir, true)?;
    Ok(())
}

async fn run_verification_fail(
    client: &reqwest::Client,
    runtime_base: &str,
    owner_user_id: u64,
    fixture: &FixtureRepo,
    provider: &ProviderHandle,
    out_dir: &Path,
) -> Result<()> {
    let run = start_run(
        client,
        runtime_base,
        "autopilot-desktop:vignette",
        json!({
            "policy_bundle_id": "vignette.phase0",
            "repo_path": fixture.path,
            "issue_number": 1,
            "commit": fixture.initial_commit,
        }),
    )
    .await?;

    append_event(
        client,
        runtime_base,
        run.id,
        "run.started",
        json!({"ok": true}),
        Some("run-started".to_string()),
    )
    .await?;

    let request = sandbox_request(
        &fixture.path,
        Some(fixture.initial_commit.clone()),
        vec!["cargo test".to_string()],
    );
    let job_hash = request.compute_hash().context("compute job hash")?;
    let dispatch =
        dispatch_sandbox_via_runtime(client, runtime_base, owner_user_id, &request).await?;
    if dispatch.job_hash != job_hash {
        return Err(anyhow!(
            "runtime dispatch job_hash mismatch: expected {}, got {}",
            job_hash,
            dispatch.job_hash
        ));
    }
    let selection = dispatch.selection;
    if selection.provider.provider_id != provider.state.id {
        return Err(anyhow!(
            "unexpected dispatched provider: expected {}, got {}",
            provider.state.id,
            selection.provider.provider_id
        ));
    }
    let response = dispatch.response;

    let settle_before = fetch_run(client, runtime_base, run.id).await?.events.len();
    let settlement = settle_sandbox_with_runtime(
        client,
        runtime_base,
        owner_user_id,
        run.id,
        &selection,
        &request,
        &response,
    )
    .await?;
    let expected_amount_msats = selection.provider.min_price_msats.unwrap_or(1_000);
    if settlement.job_hash != job_hash {
        return Err(anyhow!(
            "runtime settlement job_hash mismatch: expected {}, got {}",
            job_hash,
            settlement.job_hash
        ));
    }
    if settlement.amount_msats != expected_amount_msats {
        return Err(anyhow!(
            "runtime settlement amount mismatch: expected {}, got {}",
            expected_amount_msats,
            settlement.amount_msats
        ));
    }
    if settlement.verification_passed || settlement.settlement_status != "withheld" {
        return Err(anyhow!(
            "expected verification fail + settlement withheld, got passed={} status={}",
            settlement.verification_passed,
            settlement.settlement_status
        ));
    }
    if settlement.exit_code == 0 {
        return Err(anyhow!(
            "expected non-zero exit_code for verification-fail scenario"
        ));
    }

    let _settlement_retry = settle_sandbox_with_runtime(
        client,
        runtime_base,
        owner_user_id,
        run.id,
        &selection,
        &request,
        &response,
    )
    .await?;
    let settle_after = fetch_run(client, runtime_base, run.id).await?.events.len();
    if settle_after != settle_before + 5 {
        return Err(anyhow!(
            "unexpected settlement event count delta: before={} after={}",
            settle_before,
            settle_after
        ));
    }

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "ProviderQuarantined",
        json!({"provider_id": provider.state.id, "job_hash": job_hash, "reason": "verification_failed"}),
        Some("provider-quarantine".to_string()),
    )
    .await?;

    append_event(
        client,
        runtime_base,
        run.id,
        "run.finished",
        json!({"status": "failed"}),
        Some("run-finished".to_string()),
    )
    .await?;

    write_bundle(
        client,
        runtime_base,
        run.id,
        out_dir,
        "# PR Summary (Vignette Phase 0)\n\nVerification intentionally failed.\n",
    )
    .await?;
    write_bridge_events(
        out_dir,
        runtime_base,
        run.id,
        &provider.state.id,
        &job_hash,
        "withheld",
    )?;
    write_metrics(out_dir, false)?;
    Ok(())
}

async fn run_provider_offline_mid_run(
    client: &reqwest::Client,
    runtime_base: &str,
    owner_user_id: u64,
    fixture: &FixtureRepo,
    provider_a: &ProviderHandle,
    provider_b: &ProviderHandle,
    out_dir: &Path,
) -> Result<()> {
    let repo = clone_fixture_repo(fixture, &out_dir.join("repo"))?;
    let (branch, commit) = apply_issue_fix(&repo)?;
    let run = start_run(
        client,
        runtime_base,
        "autopilot-desktop:vignette",
        json!({
            "policy_bundle_id": "vignette.phase0",
            "repo_path": repo.path,
            "issue_number": 1,
            "branch": branch,
            "commit": commit,
        }),
    )
    .await?;

    append_event(
        client,
        runtime_base,
        run.id,
        "run.started",
        json!({"ok": true}),
        Some("run-started".to_string()),
    )
    .await?;

    let request = sandbox_request(
        &repo.path,
        Some(commit.clone()),
        vec![
            "sleep 2 && echo step1".to_string(),
            "cargo test".to_string(),
        ],
    );
    let job_hash = request.compute_hash().context("compute job hash")?;

    // Disable provider A while it is mid-run (during sleep).
    let disable_url = format!("{}/v1/disable", provider_a.base_url);
    let disable_task = {
        let client = client.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let _ = client.post(disable_url).send().await;
        })
    };

    let dispatch =
        dispatch_sandbox_via_runtime(client, runtime_base, owner_user_id, &request).await?;
    let _ = disable_task.await;
    if dispatch.job_hash != job_hash {
        return Err(anyhow!(
            "runtime dispatch job_hash mismatch: expected {}, got {}",
            job_hash,
            dispatch.job_hash
        ));
    }
    if dispatch.selection.provider.provider_id != provider_b.state.id {
        return Err(anyhow!(
            "expected reserve provider fallback to {}, got {}",
            provider_b.state.id,
            dispatch.selection.provider.provider_id
        ));
    }
    if dispatch.fallback_from_provider_id.as_deref() != Some(provider_a.state.id.as_str()) {
        return Err(anyhow!(
            "expected fallback_from_provider_id to be provider A"
        ));
    }

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "ProviderQuarantined",
        json!({"provider_id": provider_a.state.id, "job_hash": job_hash, "reason": "offline_mid_run"}),
        Some("provider-a-quarantine".to_string()),
    )
    .await?;

    let DispatchSandboxRunRuntimeResponse {
        selection,
        response: response_b,
        latency_ms,
        ..
    } = dispatch;

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "JobDispatched",
        json!({"job_type": SandboxRunRequest::JOB_TYPE, "job_hash": job_hash, "provider_id": selection.provider.provider_id, "idempotency_key": job_hash}),
        Some(format!("job-dispatched:{job_hash}")),
    )
    .await?;
    append_tool_event(
        client,
        runtime_base,
        run.id,
        "oa.sandbox_run.v1",
        &request,
        &response_b,
        latency_ms,
    )
    .await?;
    let settle_before = fetch_run(client, runtime_base, run.id).await?.events.len();
    let settlement = settle_sandbox_with_runtime(
        client,
        runtime_base,
        owner_user_id,
        run.id,
        &selection,
        &request,
        &response_b,
    )
    .await?;
    let expected_amount_msats = selection.provider.min_price_msats.unwrap_or(1_000);
    if settlement.job_hash != job_hash {
        return Err(anyhow!(
            "runtime settlement job_hash mismatch: expected {}, got {}",
            job_hash,
            settlement.job_hash
        ));
    }
    if settlement.amount_msats != expected_amount_msats {
        return Err(anyhow!(
            "runtime settlement amount mismatch: expected {}, got {}",
            expected_amount_msats,
            settlement.amount_msats
        ));
    }
    if !settlement.verification_passed || settlement.settlement_status != "released" {
        return Err(anyhow!(
            "expected verification pass + settlement release, got passed={} status={}",
            settlement.verification_passed,
            settlement.settlement_status
        ));
    }
    if settlement.exit_code != 0 {
        return Err(anyhow!(
            "expected exit_code=0 for fallback scenario, got {}",
            settlement.exit_code
        ));
    }

    let _settlement_retry = settle_sandbox_with_runtime(
        client,
        runtime_base,
        owner_user_id,
        run.id,
        &selection,
        &request,
        &response_b,
    )
    .await?;
    let settle_after = fetch_run(client, runtime_base, run.id).await?.events.len();
    if settle_after != settle_before + 5 {
        return Err(anyhow!(
            "unexpected settlement event count delta: before={} after={}",
            settle_before,
            settle_after
        ));
    }

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "FallbackProviderUsed",
        json!({"from": provider_a.state.id, "to": provider_b.state.id, "job_hash": job_hash}),
        Some("fallback-used".to_string()),
    )
    .await?;

    append_event(
        client,
        runtime_base,
        run.id,
        "run.finished",
        json!({"status": "succeeded"}),
        Some("run-finished".to_string()),
    )
    .await?;

    write_bundle(client, runtime_base, run.id, out_dir, &format!(
        "# PR Summary (Vignette Phase 0)\n\nProvider A went offline mid-run; fallback to Provider B succeeded.\n\n- Commit: {commit}\n"
    ))
    .await?;
    write_bridge_events(
        out_dir,
        runtime_base,
        run.id,
        &provider_b.state.id,
        &job_hash,
        "released",
    )?;
    write_metrics(out_dir, true)?;

    // Re-enable provider A for subsequent tests.
    let enable_url = format!("{}/v1/enable", provider_a.base_url);
    let _ = client.post(enable_url).send().await;
    Ok(())
}

async fn run_emergency_disable(
    client: &reqwest::Client,
    runtime_base: &str,
    _owner_user_id: u64,
    fixture: &FixtureRepo,
    provider: &ProviderHandle,
    out_dir: &Path,
) -> Result<()> {
    let run = start_run(
        client,
        runtime_base,
        "autopilot-desktop:vignette",
        json!({
            "policy_bundle_id": "vignette.phase0",
            "repo_path": fixture.path,
            "issue_number": 1
        }),
    )
    .await?;

    append_event(
        client,
        runtime_base,
        run.id,
        "run.started",
        json!({"ok": true}),
        Some("run-started".to_string()),
    )
    .await?;

    // Emergency disable provider.
    let disable_url = format!("{}/v1/disable", provider.base_url);
    let resp = client
        .post(disable_url)
        .send()
        .await
        .context("disable provider")?;
    if !resp.status().is_success() {
        return Err(anyhow!("failed to disable provider for emergency test"));
    }

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "ProviderDisabled",
        json!({"provider_id": provider.state.id}),
        Some("provider-disabled".to_string()),
    )
    .await?;

    // New job submissions must be blocked immediately.
    let request = sandbox_request(
        &fixture.path,
        Some(fixture.initial_commit.clone()),
        vec!["cargo test".to_string()],
    );
    let job_hash = request.compute_hash().context("compute job hash")?;

    let result = call_sandbox_provider(client, &provider.base_url, &request, &job_hash).await;
    if result.is_ok() {
        return Err(anyhow!(
            "emergency-disable scenario unexpectedly accepted new work"
        ));
    }

    append_receipt_event(
        client,
        runtime_base,
        run.id,
        "JobRejected",
        json!({"provider_id": provider.state.id, "job_hash": job_hash, "reason": "provider_disabled"}),
        Some("job-rejected".to_string()),
    )
    .await?;

    append_event(
        client,
        runtime_base,
        run.id,
        "run.finished",
        json!({"status": "succeeded"}),
        Some("run-finished".to_string()),
    )
    .await?;

    write_bundle(
        client,
        runtime_base,
        run.id,
        out_dir,
        "# PR Summary (Vignette Phase 0)\n\nEmergency disable blocks new jobs.\n",
    )
    .await?;
    write_bridge_events(
        out_dir,
        runtime_base,
        run.id,
        &provider.state.id,
        &job_hash,
        "withheld",
    )?;
    write_metrics(out_dir, true)?;

    // Re-enable for safety if operator wants to keep using provider afterwards.
    let enable_url = format!("{}/v1/enable", provider.base_url);
    let _ = client.post(enable_url).send().await;
    Ok(())
}

fn sandbox_request(
    repo_path: &Path,
    git_ref: Option<String>,
    commands: Vec<String>,
) -> SandboxRunRequest {
    let commands = commands
        .into_iter()
        .map(|cmd| protocol::jobs::sandbox::SandboxCommand::new(cmd))
        .collect::<Vec<_>>();

    SandboxRunRequest {
        sandbox: SandboxConfig {
            provider: "local".to_string(),
            image_digest: "host".to_string(),
            network_policy: NetworkPolicy::None,
            resources: ResourceLimits {
                memory_mb: 512,
                cpus: 1.0,
                timeout_secs: 60,
                disk_mb: None,
            },
        },
        repo: protocol::jobs::sandbox::RepoMount {
            source: repo_path.to_string_lossy().to_string(),
            git_ref,
            mount_path: "/workspace".to_string(),
        },
        commands,
        env: HashMap::new(),
        verification: protocol::verification::Verification::objective(),
    }
}

#[derive(Debug, Deserialize)]
struct SandboxSettleResponse {
    job_hash: String,
    reservation_id: String,
    amount_msats: u64,
    verification_passed: bool,
    exit_code: i32,
    #[serde(default)]
    violations: Vec<String>,
    settlement_status: String,
}

async fn settle_sandbox_with_runtime(
    client: &reqwest::Client,
    runtime_base: &str,
    owner_user_id: u64,
    run_id: uuid::Uuid,
    selection: &ProviderSelection,
    request: &SandboxRunRequest,
    response: &SandboxRunResponse,
) -> Result<SandboxSettleResponse> {
    let url = format!("{runtime_base}/internal/v1/treasury/compute/settle/sandbox-run");
    let amount_msats = selection.provider.min_price_msats.unwrap_or(1_000);
    let resp = client
        .post(url)
        .json(&json!({
            "owner_user_id": owner_user_id,
            "run_id": run_id,
            "provider_id": selection.provider.provider_id,
            "provider_worker_id": selection.provider.worker_id,
            "amount_msats": amount_msats,
            "request": request,
            "response": response,
        }))
        .send()
        .await
        .context("settle sandbox run with runtime")?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("runtime sandbox settlement failed: {}", text));
    }

    let parsed: SandboxSettleResponse = resp.json().await.context("parse sandbox settle")?;
    Ok(parsed)
}

#[derive(Debug, Deserialize)]
struct DispatchSandboxRunRuntimeResponse {
    job_hash: String,
    selection: ProviderSelection,
    response: SandboxRunResponse,
    latency_ms: u64,
    #[serde(default)]
    fallback_from_provider_id: Option<String>,
}

async fn dispatch_sandbox_via_runtime(
    client: &reqwest::Client,
    runtime_base: &str,
    owner_user_id: u64,
    request: &SandboxRunRequest,
) -> Result<DispatchSandboxRunRuntimeResponse> {
    let url = format!("{runtime_base}/internal/v1/marketplace/dispatch/sandbox-run");
    let resp = client
        .post(url)
        .json(&json!({ "owner_user_id": owner_user_id, "request": request }))
        .send()
        .await
        .context("dispatch sandbox via runtime")?;
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("dispatch sandbox via runtime failed: {}", text));
    }
    resp.json::<DispatchSandboxRunRuntimeResponse>()
        .await
        .context("parse dispatch response")
}

async fn call_sandbox_provider(
    client: &reqwest::Client,
    provider_base: &str,
    request: &SandboxRunRequest,
    idempotency_key: &str,
) -> Result<SandboxRunResponse> {
    let url = format!("{provider_base}/v1/sandbox_run");
    let resp = client
        .post(url)
        .header("x-idempotency-key", idempotency_key)
        .json(request)
        .send()
        .await
        .context("call sandbox provider")?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("provider error: {}", text));
    }

    resp.json().await.context("parse SandboxRunResponse")
}

fn assert_same_sandbox_response(
    left: &SandboxRunResponse,
    right: &SandboxRunResponse,
) -> Result<()> {
    let left_hash = canonical_hash(left).context("hash left response")?;
    let right_hash = canonical_hash(right).context("hash right response")?;
    if left_hash != right_hash {
        return Err(anyhow!("idempotent response mismatch"));
    }
    Ok(())
}

async fn assert_provider_idempotency(
    client: &reqwest::Client,
    provider: &ProviderHandle,
    expected_executions: u64,
    expected_cache_hits: u64,
) -> Result<()> {
    let url = format!("{}/v1/status", provider.base_url);
    let resp = client.get(url).send().await.context("provider status")?;
    if !resp.status().is_success() {
        return Err(anyhow!("failed to fetch provider status"));
    }
    let status: ProviderStatusResponse = resp.json().await.context("parse provider status")?;
    if status.executions != expected_executions {
        return Err(anyhow!(
            "provider executions mismatch: expected {}, got {}",
            expected_executions,
            status.executions
        ));
    }
    if status.cache_hits != expected_cache_hits {
        return Err(anyhow!(
            "provider cache_hits mismatch: expected {}, got {}",
            expected_cache_hits,
            status.cache_hits
        ));
    }
    Ok(())
}

async fn start_run(
    client: &reqwest::Client,
    runtime_base: &str,
    worker_id: &str,
    metadata: Value,
) -> Result<RuntimeRun> {
    let url = format!("{runtime_base}/internal/v1/runs");
    let resp = client
        .post(url)
        .json(&json!({"worker_id": worker_id, "metadata": metadata}))
        .send()
        .await
        .context("start run")?;
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("start run failed: {}", text));
    }
    let parsed: RunResponse = resp.json().await.context("parse run response")?;
    Ok(parsed.run)
}

async fn append_event(
    client: &reqwest::Client,
    runtime_base: &str,
    run_id: uuid::Uuid,
    event_type: &str,
    payload: Value,
    idempotency_key: Option<String>,
) -> Result<()> {
    let url = format!("{runtime_base}/internal/v1/runs/{run_id}/events");
    let resp = client
        .post(url)
        .json(&json!({
            "event_type": event_type,
            "payload": payload,
            "idempotency_key": idempotency_key,
        }))
        .send()
        .await
        .context("append run event")?;
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("append event failed: {}", text));
    }
    Ok(())
}

async fn fetch_run(
    client: &reqwest::Client,
    runtime_base: &str,
    run_id: uuid::Uuid,
) -> Result<RuntimeRun> {
    let url = format!("{runtime_base}/internal/v1/runs/{run_id}");
    let resp = client.get(url).send().await.context("get run")?;
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("get run failed: {}", text));
    }
    let parsed: RunResponse = resp.json().await.context("parse run response")?;
    Ok(parsed.run)
}

async fn assert_runtime_idempotent_key(
    client: &reqwest::Client,
    runtime_base: &str,
    run_id: uuid::Uuid,
    event_type: &str,
    payload: Value,
    idempotency_key: String,
) -> Result<()> {
    let before = fetch_run(client, runtime_base, run_id).await?.events.len();
    append_event(
        client,
        runtime_base,
        run_id,
        event_type,
        payload,
        Some(idempotency_key),
    )
    .await?;
    let after = fetch_run(client, runtime_base, run_id).await?.events.len();
    if after != before {
        return Err(anyhow!(
            "idempotency violated: expected event count {}, got {}",
            before,
            after
        ));
    }
    Ok(())
}

async fn append_receipt_event(
    client: &reqwest::Client,
    runtime_base: &str,
    run_id: uuid::Uuid,
    receipt_type: &str,
    payload: Value,
    idempotency_key: Option<String>,
) -> Result<()> {
    append_event(
        client,
        runtime_base,
        run_id,
        "receipt",
        json!({"receipt_type": receipt_type, "payload": payload}),
        idempotency_key,
    )
    .await
}

async fn append_tool_event(
    client: &reqwest::Client,
    runtime_base: &str,
    run_id: uuid::Uuid,
    tool: &str,
    request: &SandboxRunRequest,
    response: &SandboxRunResponse,
    latency_ms: u64,
) -> Result<()> {
    let params_hash = request
        .compute_hash()
        .unwrap_or_else(|_| "sha256:invalid".to_string());
    let output_hash = canonical_hash(response).unwrap_or_else(|_| "sha256:invalid".to_string());
    append_event(
        client,
        runtime_base,
        run_id,
        "tool",
        json!({
            "tool": tool,
            "params_hash": params_hash,
            "output_hash": output_hash,
            "latency_ms": latency_ms,
            "side_effects": []
        }),
        Some(format!("tool:{tool}")),
    )
    .await
}

async fn write_bundle(
    client: &reqwest::Client,
    runtime_base: &str,
    run_id: uuid::Uuid,
    out_dir: &Path,
    pr_summary: &str,
) -> Result<()> {
    let receipt_url = format!("{runtime_base}/internal/v1/runs/{run_id}/receipt");
    let receipt = client
        .get(receipt_url)
        .send()
        .await
        .context("fetch receipt")?
        .error_for_status()
        .context("receipt status")?
        .text()
        .await
        .context("receipt body")?;

    let replay_url = format!("{runtime_base}/internal/v1/runs/{run_id}/replay");
    let replay = client
        .get(replay_url)
        .send()
        .await
        .context("fetch replay")?
        .error_for_status()
        .context("replay status")?
        .text()
        .await
        .context("replay body")?;

    std::fs::write(out_dir.join("PR_SUMMARY.md"), pr_summary)?;
    std::fs::write(out_dir.join("RECEIPT.json"), receipt)?;
    std::fs::write(out_dir.join("REPLAY.jsonl"), replay)?;

    // Basic assertions: files exist and replay is parseable jsonl.
    assert_replay_jsonl(out_dir.join("REPLAY.jsonl"))?;
    Ok(())
}

fn assert_replay_jsonl(path: PathBuf) -> Result<()> {
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    openagents_runtime_service::artifacts::validate_replay_jsonl(&content)
        .map_err(|err| anyhow!("invalid REPLAY.jsonl: {}", err))?;
    Ok(())
}

fn write_bridge_events(
    out_dir: &Path,
    runtime_base: &str,
    run_id: Uuid,
    provider_id: &str,
    job_hash: &str,
    settlement_status: &str,
) -> Result<()> {
    let receipt_json =
        std::fs::read_to_string(out_dir.join("RECEIPT.json")).context("read RECEIPT.json")?;
    let receipt_sha256 = receipt_sha256_from_utf8(&receipt_json);
    let receipt_url = format!("{runtime_base}/internal/v1/runs/{run_id}/receipt");

    let provider_ad = ProviderAdV1 {
        provider_id: provider_id.to_string(),
        name: "OpenAgents Compute Provider".to_string(),
        description: "Phase-0 vignette provider ad".to_string(),
        website: Some("https://openagents.com".to_string()),
        capabilities: vec!["oa.sandbox_run.v1".to_string()],
        min_price_msats: 1000,
    };

    let receipt_ptr = ReceiptPointerV1 {
        provider_id: provider_id.to_string(),
        run_id: run_id.to_string(),
        job_hash: job_hash.to_string(),
        receipt_sha256,
        settlement_status: settlement_status.to_string(),
        receipt_url,
    };

    // Deterministic harness secret, so output is stable across runs for the same provider id.
    let mut hasher = Sha256::new();
    hasher.update(b"openagents:vignette:bridge:");
    hasher.update(provider_id.as_bytes());
    let digest = hasher.finalize();
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&digest[..32]);

    let provider_event = build_provider_ad_event(&secret, None, &provider_ad)?;
    let receipt_event = build_receipt_pointer_event(&secret, None, &receipt_ptr)?;

    // Ensure emitted events are Phase-0 boundary compliant and Nostr-verifiable.
    validate_phase0_bridge_event(&provider_event)?;
    validate_phase0_bridge_event(&receipt_event)?;

    let mut out = String::new();
    for event in [provider_event, receipt_event] {
        out.push_str(&serde_json::to_string(&event)?);
        out.push('\n');
    }
    std::fs::write(out_dir.join("bridge_nostr_events.jsonl"), out)?;
    Ok(())
}

fn write_metrics(out_dir: &Path, succeeded: bool) -> Result<()> {
    let metrics = json!({
        "fill_rate": if succeeded { 1.0 } else { 0.0 },
        "median_latency_ms": 0,
        "effective_cost_msats": 1000,
        "provider_breadth": 1,
        "verification_pass_rate": if succeeded { 1.0 } else { 0.0 },
        "rework_rate": 0.0,
        "caps_enforced": true
    });
    std::fs::write(
        out_dir.join("metrics.json"),
        serde_json::to_string_pretty(&metrics)?,
    )?;
    Ok(())
}
