//! `openagents gym run` — execute a suite through the Gym lifecycle.
//!
//! This is the Rust control plane over the existing host-native Harbor path.
//! The container and Box targets are intentionally behind one function so the
//! next lanes can plug in without rewriting the lifecycle.

use crate::computer::now_iso8601;
use crate::errors::CliError;
use crate::gym::env::{
    HARBOR_PULL_REMEDY, HARBOR_RUNNER_IMAGE, diagnose, harbor_runner_image_present,
    probe_environment,
};
use crate::gym::schemas::{RUN_STATUS_SCHEMA, RunStatus, RunTrial};
use crate::gym::suite::{ResolvedSuite, resolve_for_run};
use crate::tracker::{ApiError, error_fields, error_sentence, header_request_id};
use clap::{Args, Subcommand};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub const AGENT_NAME: &str = "openagents-coder";
pub const DEFAULT_API_ORIGIN: &str = "http://localhost:4000";

/// Arguments for `openagents gym run`.
#[derive(Args, Debug)]
pub struct RunArgs {
    #[command(subcommand)]
    pub action: RunAction,
}

#[derive(Subcommand, Debug)]
pub enum RunAction {
    /// Execute a suite.
    Run(ExecuteArgs),
    /// Read a registered run's status.
    Status {
        /// Run id returned by `gym run`.
        run_id: String,
    },
    /// List recent runs.
    List {
        /// Show only runs owned by the caller.
        #[arg(long)]
        mine: bool,
    },
    /// Cancel a run.
    Cancel {
        /// Run id.
        run_id: String,
    },
}

/// Flags for `openagents gym run <suite-id>`.
#[derive(Args, Debug)]
pub struct ExecuteArgs {
    /// Suite id (the `id` field in `bench/suites/<id>.suite.json`).
    pub suite_id: String,

    /// Harbor model string, e.g. `openai/gpt-5.6-luna`.
    #[arg(short, long)]
    pub model: String,

    /// Gym lane. Inferred from the model string when omitted.
    #[arg(long, value_parser = ["proxy", "local"])]
    pub lane: Option<String>,

    /// Number of concurrent trials.
    #[arg(long, default_value_t = 1)]
    pub n_concurrent: usize,

    /// Harbor jobs directory. A fresh timestamped directory under `/tmp` when omitted.
    #[arg(long)]
    pub jobs_dir: Option<PathBuf>,

    /// Timeout multiplier passed through to Harbor.
    #[arg(long)]
    pub timeout_multiplier: Option<f64>,

    /// Harbor environment provider passed through to the runner.
    #[arg(long)]
    pub env: Option<String>,

    /// Print the lifecycle calls and exit without registering or executing.
    #[arg(long)]
    pub dry_run: bool,
}

/// HTTP client for the Gym lifecycle API.
#[derive(Debug, Clone)]
pub struct GymClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

/// What `POST /api/v1/gym/runs/start` answered.
#[derive(Debug, Clone, Deserialize)]
pub struct StartedRun {
    pub run_id: String,
    #[serde(flatten)]
    pub extra: Value,
}

impl GymClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(token) = &self.token {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) {
                map.insert(AUTHORIZATION, value);
            }
        }
        map
    }

    /// Register a run and return the server-assigned id.
    pub async fn start_run(
        &self,
        suite: &str,
        model: &str,
        lane: &str,
        tasks_total: usize,
    ) -> Result<StartedRun, ApiError> {
        let url = format!("{}/gym/runs/start", self.api_base);
        let body = serde_json::json!({
            "suite": suite,
            "agent": AGENT_NAME,
            "model": model,
            "lane": lane,
            "tasks_total": tasks_total,
        });
        let (status, value) = self
            .request("register a gym run", "POST", &url, Some(body), &[200, 201])
            .await?;

        let run_id = value
            .get("run")
            .and_then(|r| r.get("id"))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| value.get("id").and_then(Value::as_str).map(String::from))
            .ok_or_else(|| ApiError::Malformed {
                operation: "register a gym run".to_string(),
                why: "the server accepted the run but named no id".to_string(),
            })?;

        Ok(StartedRun {
            run_id,
            extra: serde_json::json!({ "status": status }),
        })
    }

    /// Upsert the final state of one trial.
    pub async fn upsert_trial(
        &self,
        run_id: &str,
        task: &str,
        state: &str,
        thread_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let url = format!("{}/gym/runs/{}/trials", self.api_base, run_id);
        let mut body = serde_json::json!({
            "task": task,
            "state": state,
        });
        if let Some(id) = thread_id {
            body["thread_id"] = Value::String(id.to_string());
        }
        self.request("upsert a trial", "POST", &url, Some(body), &[200, 201, 204])
            .await?;
        Ok(())
    }

    /// Patch a run's top-level state.
    pub async fn patch_run(&self, run_id: &str, body: Value) -> Result<Value, ApiError> {
        let url = format!("{}/gym/runs/{}", self.api_base, run_id);
        self.request(
            "finalize a gym run",
            "PATCH",
            &url,
            Some(body),
            &[200, 201, 204],
        )
        .await
        .map(|(_, v)| v)
    }

    /// Read one registered run.
    pub async fn get_run(&self, run_id: &str) -> Result<RunStatus, ApiError> {
        let url = format!("{}/gym/runs/{}", self.api_base, run_id);
        let (_, value) = self
            .request("read a gym run", "GET", &url, None, &[200])
            .await?;
        let run = value.get("run").unwrap_or(&value);
        serde_json::from_value(run.clone()).map_err(|e| ApiError::Malformed {
            operation: "read a gym run".to_string(),
            why: e.to_string(),
        })
    }

    /// List recent runs.
    pub async fn list_runs(&self, mine: bool) -> Result<Vec<RunStatus>, ApiError> {
        let mut url = format!("{}/gym/runs", self.api_base);
        if mine {
            url.push_str("?mine=true");
        }
        let (_, value) = self
            .request("list gym runs", "GET", &url, None, &[200])
            .await?;
        let runs = value.get("runs").unwrap_or(&value);
        serde_json::from_value(runs.clone()).map_err(|e| ApiError::Malformed {
            operation: "list gym runs".to_string(),
            why: e.to_string(),
        })
    }

    /// Cancel a run by patching its status. The lifecycle route may not exist yet;
    /// the implementation reports the server's own answer.
    pub async fn cancel_run(&self, run_id: &str) -> Result<String, ApiError> {
        let url = format!("{}/gym/runs/{}", self.api_base, run_id);
        let body = serde_json::json!({ "status": "cancelled" });
        let (_, value) = self
            .request(
                "cancel a gym run",
                "PATCH",
                &url,
                Some(body),
                &[200, 201, 204],
            )
            .await?;
        let status = value
            .get("run")
            .and_then(|r| r.get("status"))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| {
                value
                    .get("status")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| "cancelled".to_string());
        Ok(status)
    }

    async fn request(
        &self,
        operation: &str,
        method: &str,
        url: &str,
        body: Option<Value>,
        accepted: &[u16],
    ) -> Result<(u16, Value), ApiError> {
        let mut builder = match method {
            "GET" => self.http.get(url),
            "POST" => self.http.post(url),
            "PATCH" => self.http.patch(url),
            "DELETE" => self.http.delete(url),
            other => {
                return Err(ApiError::Input(format!(
                    "{} is not an HTTP method this client sends.",
                    other
                )));
            }
        };
        builder = builder.headers(self.headers());
        if let Some(body) = body {
            builder = builder.json(&body);
        }

        crate::diag::request(method, url);
        let response = builder.send().await.map_err(|e| {
            crate::diag::transport(url, &e.to_string());
            ApiError::Transport {
                operation: operation.to_string(),
                why: e.to_string(),
            }
        })?;

        let status = response.status().as_u16();
        crate::diag::response(status, url);
        let header_id = header_request_id(&response);
        let text = response.text().await.map_err(|e| ApiError::Transport {
            operation: operation.to_string(),
            why: e.to_string(),
        })?;

        if !accepted.contains(&status) {
            let message = error_sentence(&text, status);
            crate::diag::refused(status, &message);
            let (code, body_id) = error_fields(&text);
            return Err(ApiError::Refused {
                operation: operation.to_string(),
                status,
                message,
                code,
                request_id: header_id.or(body_id),
            });
        }

        if text.trim().is_empty() {
            return Ok((status, Value::Null));
        }
        let value = serde_json::from_str(&text).map_err(|e| ApiError::Malformed {
            operation: operation.to_string(),
            why: e.to_string(),
        })?;
        Ok((status, value))
    }
}

/// Dispatch `openagents gym run`.
pub async fn run(
    action: RunAction,
    api_base: &str,
    token: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    match action {
        RunAction::Run(args) => execute(args, api_base, token, json).await,
        RunAction::Status { run_id } => {
            let client = GymClient::new(api_base, token);
            let status = client.get_run(&run_id).await.map_err(api_to_cli)?;
            emit_run_status(&status, json);
            Ok(())
        }
        RunAction::List { mine } => {
            let client = GymClient::new(api_base, token);
            let runs = client.list_runs(mine).await.map_err(api_to_cli)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "schema": RUN_STATUS_SCHEMA,
                        "runs": runs,
                    }))
                    .map_err(|e| CliError::Output(e.to_string()))?
                );
            } else {
                for run in &runs {
                    println!("{}", crate::gym::views::render_run_list_line(run));
                }
            }
            Ok(())
        }
        RunAction::Cancel { run_id } => {
            let client = GymClient::new(api_base, token);
            let status = client.cancel_run(&run_id).await.map_err(api_to_cli)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "run_id": run_id,
                        "status": status,
                    }))
                    .map_err(|e| CliError::Output(e.to_string()))?
                );
            } else {
                println!("run {run_id} status={status}");
            }
            Ok(())
        }
    }
}

async fn execute(
    args: ExecuteArgs,
    api_base: &str,
    token: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    // 1. Resolve the suite and refuse drifted pins before anything else.
    let suite = resolve_for_run(&args.suite_id)?;

    // 2. Decide the lane and the catalog model the server expects.
    let lane = args.lane.clone().unwrap_or_else(|| infer_lane(&args.model));
    let catalog_model = catalog_model(&args.model);

    // 3. Dry run: print the exact lifecycle calls and stop, registering nothing.
    if args.dry_run {
        print_dry_run_plan(&suite, &args.model, &catalog_model, &lane, &args);
        return Ok(());
    }

    // 4. Verify prereqs and print the fix for every failure.
    let checks = probe_environment().await;
    let (lines, exit) = diagnose(&checks);
    if exit != 0 {
        for line in &lines {
            println!("{line}");
        }
        return Err(CliError::Internal(format!(
            "gym run cannot start: the machine is not ready ({} check(s) failed)",
            checks
                .iter()
                .filter(|c| !matches!(c.state, crate::gym::env::CheckState::Ok))
                .count()
        )));
    }

    // 5. Register when a token is present; say so when not — never silently.
    //    `bench/run-suite.sh` owns registration when it is the execution
    //    target, so this client does not start a second run beside it.
    let client = GymClient::new(api_base, token.clone());
    let use_suite_script = will_use_suite_script();
    let registration = if use_suite_script {
        println!("run registration: deferred to bench/run-suite.sh");
        None
    } else if let Some(_token) = &token {
        let started = client
            .start_run(&suite.id, &catalog_model, &lane, suite.tasks.len())
            .await
            .map_err(api_to_cli)?;
        println!("run registered: {}", started.run_id);
        Some(started.run_id)
    } else {
        println!("run unregistered: no stored token found");
        None
    };

    // 6. Execute: pinned container first, host-native Harbor as the fallback.
    let jobs_dir = match args.jobs_dir {
        Some(p) => p,
        None => {
            let stamp = now_iso8601().replace([':', '.'], "-");
            PathBuf::from(format!("/tmp/gym-jobs-{}", stamp))
        }
    };
    let _ = tokio::fs::create_dir_all(&jobs_dir).await.map_err(|e| {
        CliError::Internal(format!(
            "could not create jobs directory {}: {e}",
            jobs_dir.display()
        ))
    })?;

    let run_id = registration.as_deref();
    let host = HostPlan {
        suite,
        model: args.model,
        lane,
        n_concurrent: args.n_concurrent,
        jobs_dir: jobs_dir.clone(),
        timeout_multiplier: args.timeout_multiplier,
        env_provider: args.env,
        run_id: run_id.map(String::from),
        api_base: api_base.to_string(),
        token,
    };

    let job_dir = match run_on_target(&host).await {
        Ok(dir) => Some(dir),
        Err(e) => {
            eprintln!("harbor run failed: {e}");
            if let Some(run_id) = run_id {
                client
                    .patch_run(run_id, serde_json::json!({"status": "abandoned"}))
                    .await
                    .map_err(api_to_cli)?;
                println!("run {run_id} abandoned: harbor run failed before grading");
            }
            return Err(e);
        }
    };

    // 7. Finalize. A crashed verifier means no trial graded: patch `abandoned`.
    let job_dir = job_dir.ok_or_else(|| {
        CliError::Internal("harbor run did not produce a job directory".to_string())
    })?;
    let status = finalize_job_dir(
        &client,
        run_id,
        &host.suite,
        &host.lane,
        &host.model,
        &job_dir,
    )
    .await?;

    emit_run_status(&status, json);
    Ok(())
}

/// A plan for one Harbor run. Container and host-native targets share it.
#[derive(Debug, Clone)]
pub struct HostPlan {
    pub suite: ResolvedSuite,
    pub model: String,
    pub lane: String,
    pub n_concurrent: usize,
    pub jobs_dir: PathBuf,
    pub timeout_multiplier: Option<f64>,
    pub env_provider: Option<String>,
    pub run_id: Option<String>,
    pub api_base: String,
    pub token: Option<String>,
}

const CONTAINER_JOBS_DIR: &str = "/jobs";
const DOCKER_SOCKET: &str = "/var/run/docker.sock";

fn prefer_container_target() -> bool {
    std::env::var("OPENAGENTS_GYM_RUN_TARGET")
        .map(|v| v.eq_ignore_ascii_case("container"))
        .unwrap_or(false)
}

fn suite_script_path() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let script = cwd.join("bench").join("run-suite.sh");
    script.is_file().then_some(script)
}

fn will_use_suite_script() -> bool {
    !prefer_container_target() && suite_script_path().is_some()
}

/// Prefer `bench/run-suite.sh` (packs the working-tree CLI, registers the
/// Gym run, invokes Harbor). The digest-pinned harbor-runner image is the
/// container target when `OPENAGENTS_GYM_RUN_TARGET=container` or the
/// script is missing. Host-native Harbor is the last fallback.
async fn run_on_target(plan: &HostPlan) -> Result<PathBuf, CliError> {
    if will_use_suite_script() {
        let script = suite_script_path().expect("will_use_suite_script implies the script");
        println!("execution target: {}", script.display());
        return run_on_suite_script(plan, &script).await;
    }
    if harbor_runner_image_present().await {
        println!("execution target: container ({HARBOR_RUNNER_IMAGE})");
        return run_on_container(plan).await;
    }
    eprintln!("execution target: host-native Harbor (image missing; {HARBOR_PULL_REMEDY})");
    run_on_host(plan).await
}

/// Drive Harbor through the same script `docs/coder/runbook.md` documents.
async fn run_on_suite_script(plan: &HostPlan, script: &Path) -> Result<PathBuf, CliError> {
    let suite_file = format!("bench/suites/{}.suite.json", plan.suite.id);
    let mut cmd = Command::new("bash");
    cmd.arg(script)
        .arg(&suite_file)
        .arg("--model")
        .arg(&plan.model)
        .arg("--lane")
        .arg(&plan.lane)
        .arg("--jobs-dir")
        .arg(&plan.jobs_dir)
        .arg("--n-concurrent")
        .arg(plan.n_concurrent.to_string())
        .arg("--api-url")
        .arg(&plan.api_base);
    if let Some(m) = plan.timeout_multiplier {
        cmd.arg("--timeout-multiplier").arg(m.to_string());
    }
    if let Some(token) = &plan.token {
        cmd.env("OPENAGENTS_TOKEN", token);
    }
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());
    let status = cmd
        .status()
        .await
        .map_err(|e| CliError::Internal(format!("could not start {}: {e}", script.display())))?;
    if !status.success() {
        return Err(CliError::Internal(format!(
            "{} exited with status {}",
            script.display(),
            status.code().unwrap_or(-1)
        )));
    }
    locate_job_dir(&plan.jobs_dir).await
}

/// Run Harbor inside the digest-pinned harbor-runner image. The job directory
/// is bind-mounted so scoring reads the same shape the host path writes.
async fn run_on_container(plan: &HostPlan) -> Result<PathBuf, CliError> {
    let jobs_host = std::fs::canonicalize(&plan.jobs_dir).unwrap_or_else(|_| plan.jobs_dir.clone());
    let docker_args = container_docker_args(plan, &jobs_host);
    let mut cmd = Command::new("docker");
    apply_container_env(&mut cmd, plan);
    cmd.args(&docker_args);

    let output = cmd.output().await.map_err(|e| {
        CliError::Internal(format!(
            "could not start docker for {HARBOR_RUNNER_IMAGE}: {e}"
        ))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Internal(format!(
            "{HARBOR_RUNNER_IMAGE} exited with status {}: {stderr}",
            output.status.code().unwrap_or(-1)
        )));
    }

    locate_job_dir(&plan.jobs_dir).await
}

/// Run Harbor on the host. Documented fallback when the image is absent.
async fn run_on_host(plan: &HostPlan) -> Result<PathBuf, CliError> {
    let repo_root = std::env::current_dir().map_err(|e| CliError::Configuration(e.to_string()))?;
    let bench_dir = repo_root.join("bench");
    let harbor_args = harbor_args(plan, &plan.jobs_dir);

    let mut cmd = Command::new("harbor");
    cmd.env("PYTHONPATH", bench_dir.to_string_lossy().to_string());
    apply_host_env(&mut cmd, plan);
    cmd.args(&harbor_args);

    let output = cmd.output().await.map_err(|e| {
        CliError::Internal(format!(
            "could not start harbor; is it installed and on PATH? {e} ({HARBOR_PULL_REMEDY} to use the container target)"
        ))
    })?;
    if !output.status.success() {
        return Err(CliError::Internal(format!(
            "harbor exited with status {}",
            output.status.code().unwrap_or(-1)
        )));
    }

    locate_job_dir(&plan.jobs_dir).await
}

fn apply_host_env(cmd: &mut Command, plan: &HostPlan) {
    if let Some(token) = &plan.token {
        cmd.env("OPENAGENTS_TOKEN", token);
    }
    if let Ok(binary) = std::env::var("OPENAGENTS_CODER_BINARY") {
        cmd.env("OPENAGENTS_CODER_BINARY", binary);
    }
    cmd.env("OPENAGENTS_CODER_API_URL", &plan.api_base);
    if let Some(run_id) = &plan.run_id {
        cmd.env("OPENAGENTS_GYM_RUN_ID", run_id);
        cmd.env("OPENAGENTS_GYM_API_URL", &plan.api_base);
    }
}

fn apply_container_env(cmd: &mut Command, plan: &HostPlan) {
    if let Some(token) = &plan.token {
        cmd.env("OPENAGENTS_TOKEN", token);
    }
    cmd.env(
        "OPENAGENTS_CODER_API_URL",
        coder_api_url_for_container(&plan.api_base),
    );
    if let Some(run_id) = &plan.run_id {
        cmd.env("OPENAGENTS_GYM_RUN_ID", run_id);
        cmd.env(
            "OPENAGENTS_GYM_API_URL",
            coder_api_url_for_container(&plan.api_base),
        );
    }
}

/// `docker run` argv (not including `docker` itself) for the container target.
pub fn container_docker_args(plan: &HostPlan, jobs_host: &Path) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "-v".to_string(),
        format!("{}:{CONTAINER_JOBS_DIR}", jobs_host.display()),
        "-v".to_string(),
        format!("{DOCKER_SOCKET}:{DOCKER_SOCKET}"),
        "--add-host".to_string(),
        "host.docker.internal:host-gateway".to_string(),
    ];
    if plan.token.is_some() {
        args.extend(["-e".to_string(), "OPENAGENTS_TOKEN".to_string()]);
    }
    args.extend(["-e".to_string(), "OPENAGENTS_CODER_API_URL".to_string()]);
    if plan.run_id.is_some() {
        args.extend(["-e".to_string(), "OPENAGENTS_GYM_RUN_ID".to_string()]);
        args.extend(["-e".to_string(), "OPENAGENTS_GYM_API_URL".to_string()]);
    }
    args.push(HARBOR_RUNNER_IMAGE.to_string());
    args.extend(harbor_args(plan, Path::new(CONTAINER_JOBS_DIR)));
    args
}

fn harbor_args(plan: &HostPlan, jobs_dir: &Path) -> Vec<String> {
    let mut harbor_args = vec![
        "run".to_string(),
        "--dataset".to_string(),
        first_dataset(plan),
        "--agent-import-path".to_string(),
        "adapters.openagents_coder:OpenAgentsCoder".to_string(),
        "-m".to_string(),
        plan.model.clone(),
    ];
    for t in &plan.suite.tasks {
        harbor_args.push("-i".to_string());
        harbor_args.push(t.id.clone());
    }
    harbor_args.push("--jobs-dir".to_string());
    harbor_args.push(jobs_dir.to_string_lossy().into_owned());
    harbor_args.push("--n-concurrent".to_string());
    harbor_args.push(plan.n_concurrent.to_string());
    if let Some(m) = plan.timeout_multiplier {
        harbor_args.push("--timeout-multiplier".to_string());
        harbor_args.push(m.to_string());
    }
    if let Some(env) = &plan.env_provider {
        harbor_args.push("--env".to_string());
        harbor_args.push(env.clone());
    }
    harbor_args
}

/// The dataset all tasks share. A mixed dataset suite is not supported yet.
fn first_dataset(plan: &HostPlan) -> String {
    plan.suite
        .tasks
        .first()
        .map(|t| t.dataset.clone())
        .unwrap_or_else(|| "terminal-bench@2.0".to_string())
}

/// Rewrite loopback API URLs so the adapter inside the container can reach
/// the host, matching `bench/run-suite.sh`.
pub fn coder_api_url_for_container(api_base: &str) -> String {
    let mut out = api_base.to_string();
    for host in ["localhost", "127.0.0.1"] {
        let http = format!("http://{host}");
        let https = format!("https://{host}");
        if let Some(rest) = out.strip_prefix(&http) {
            out = format!("http://host.docker.internal{rest}");
            break;
        }
        if let Some(rest) = out.strip_prefix(&https) {
            out = format!("https://host.docker.internal{rest}");
            break;
        }
    }
    out
}

async fn locate_job_dir(root: &Path) -> Result<PathBuf, CliError> {
    if root.join("result.json").is_file() && root.join("config.json").is_file() {
        return Ok(root.to_path_buf());
    }
    let mut entries = tokio::fs::read_dir(root).await.map_err(|e| {
        CliError::Internal(format!(
            "could not read jobs directory {}: {e}",
            root.display()
        ))
    })?;
    let mut candidates = Vec::new();
    while let Some(entry) = entries.next_entry().await.transpose() {
        let entry = entry.map_err(|e| CliError::Internal(e.to_string()))?;
        let path = entry.path();
        if path.is_dir() && path.join("result.json").is_file() && path.join("config.json").is_file()
        {
            let meta = tokio::fs::metadata(&path).await.ok();
            if let Some(m) = meta {
                candidates.push((m.modified().ok(), path));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates
        .into_iter()
        .next()
        .map(|(_, p)| p)
        .ok_or_else(|| {
            CliError::Internal(
                "no Harbor job directory with result.json and config.json found".to_string(),
            )
        })
}

/// Read the job directory, upsert each trial, and patch the run to `graded` or
/// `abandoned` depending on whether the verifier graded anything.
pub async fn finalize_job_dir(
    client: &GymClient,
    run_id: Option<&str>,
    suite: &ResolvedSuite,
    lane: &str,
    model: &str,
    job_dir: &Path,
) -> Result<RunStatus, CliError> {
    let results = read_trial_results(job_dir).await?;
    let mut trials = Vec::with_capacity(results.len());
    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut ungraded = 0u64;
    let mut graded = 0u64;

    for result in &results {
        let trial_state = trial_state(result);
        let (display, outcome, is_graded) = match trial_state {
            TrialState::Accepted => ("accepted", Some("accepted"), true),
            TrialState::Rejected => ("rejected", Some("rejected"), true),
            TrialState::Ungraded => ("ungraded", None, false),
        };

        if is_graded {
            graded += 1;
            if matches!(trial_state, TrialState::Accepted) {
                accepted += 1;
            } else {
                rejected += 1;
            }
        } else {
            ungraded += 1;
        }

        let mut transcript_ref = None;
        if let Some(thread) = thread_id_for_trial(job_dir, &result.dir_name).await {
            transcript_ref = Some(format!("/trace/{thread}"));
        }

        let cost_usd = result
            .trajectory
            .as_ref()
            .and_then(|t| t.get("final_metrics"))
            .and_then(|m| m.get("cost_usd"))
            .and_then(Value::as_f64);

        trials.push(RunTrial {
            task: result.task.clone(),
            state: display.to_string(),
            outcome: outcome.map(String::from),
            started_at: result.started_at.clone(),
            finished_at: result.finished_at.clone(),
            transcript_ref,
            cost_usd,
        });

        if let Some(run_id) = run_id {
            let api_state = match trial_state {
                TrialState::Accepted => "passed",
                TrialState::Rejected => "failed",
                TrialState::Ungraded => "ungraded",
            };
            client
                .upsert_trial(run_id, &result.task, api_state, None)
                .await
                .map_err(api_to_cli)?;
        }
    }

    if graded == 0 {
        // A run whose verifier never graded a single trial is `abandoned`.
        if let Some(run_id) = run_id {
            client
                .patch_run(run_id, serde_json::json!({"status": "abandoned"}))
                .await
                .map_err(api_to_cli)?;
        }
        let summary = format!(
            "abandoned: no trial's verifier ran; {}/{} tasks ungraded",
            ungraded,
            suite.tasks.len()
        );
        return Ok(RunStatus {
            schema: RUN_STATUS_SCHEMA.to_string(),
            run_id: run_id.unwrap_or("local").to_string(),
            suite_id: suite.id.clone(),
            lane: lane.to_string(),
            model: Some(model.to_string()),
            state: "abandoned".to_string(),
            started_at: Some(now_iso8601()),
            updated_at: Some(now_iso8601()),
            tasks_total: suite.tasks.len() as u64,
            accepted,
            rejected,
            ungraded,
            graded,
            summary,
            trials,
        });
    }

    let summary = format!(
        "{} accepted, {} rejected, {} ungraded; {} of {} tasks graded",
        accepted,
        rejected,
        ungraded,
        graded,
        suite.tasks.len()
    );
    let report = build_report(job_dir, &results);
    let patch = serde_json::json!({
        "status": "graded",
        "tasks_total": suite.tasks.len(),
        "tasks_passed": accepted,
        "report": report,
    });
    if let Some(run_id) = run_id {
        client.patch_run(run_id, patch).await.map_err(api_to_cli)?;
    }

    Ok(RunStatus {
        schema: RUN_STATUS_SCHEMA.to_string(),
        run_id: run_id.unwrap_or("local").to_string(),
        suite_id: suite.id.clone(),
        lane: lane.to_string(),
        model: Some(model.to_string()),
        state: "graded".to_string(),
        started_at: Some(now_iso8601()),
        updated_at: Some(now_iso8601()),
        tasks_total: suite.tasks.len() as u64,
        accepted,
        rejected,
        ungraded,
        graded,
        summary,
        trials,
    })
}

fn build_report(_job_dir: &Path, results: &[TrialResult]) -> Value {
    let mut report_trials = Vec::new();
    for r in results {
        let mut entry = serde_json::json!({
            "task": r.task,
            "passed": matches!(trial_state(r), TrialState::Accepted),
        });
        if let Some(exc) = r.exception_type() {
            entry["exception"] = Value::String(exc.to_string());
        }
        report_trials.push(entry);
    }
    serde_json::json!({
        "trials": report_trials,
    })
}

#[derive(Debug, Clone)]
struct TrialResult {
    dir_name: String,
    task: String,
    result: Value,
    trajectory: Option<Value>,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl TrialResult {
    fn verifier_ran(&self) -> bool {
        self.result.get("verifier_result").is_some() && !self.result["verifier_result"].is_null()
    }

    fn exception_type(&self) -> Option<String> {
        self.result
            .get("exception_info")
            .and_then(|e| e.get("exception_type"))
            .and_then(Value::as_str)
            .map(String::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrialState {
    Accepted,
    Rejected,
    Ungraded,
}

fn trial_state(result: &TrialResult) -> TrialState {
    if !result.verifier_ran() {
        return TrialState::Ungraded;
    }
    let reward = result
        .result
        .get("verifier_result")
        .and_then(|v| v.get("rewards"))
        .and_then(|r| {
            r.as_f64().or_else(|| {
                r.as_object()
                    .and_then(|m| m.values().next())
                    .and_then(Value::as_f64)
            })
        });
    if reward.map(|r| r > 0.0).unwrap_or(false) {
        TrialState::Accepted
    } else {
        TrialState::Rejected
    }
}

async fn read_trial_results(job_dir: &Path) -> Result<Vec<TrialResult>, CliError> {
    let mut entries = tokio::fs::read_dir(job_dir).await.map_err(|e| {
        CliError::Internal(format!(
            "could not read job directory {}: {e}",
            job_dir.display()
        ))
    })?;
    let mut results = Vec::new();
    while let Some(entry) = entries.next_entry().await.transpose() {
        let entry = entry.map_err(|e| CliError::Internal(e.to_string()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let result_path = path.join("result.json");
        if !result_path.is_file() {
            continue;
        }
        let text = tokio::fs::read_to_string(&result_path).await.map_err(|e| {
            CliError::Internal(format!("could not read {}: {e}", result_path.display()))
        })?;
        let result: Value = serde_json::from_str(&text).map_err(|e| {
            CliError::Internal(format!("{} is not valid JSON: {e}", result_path.display()))
        })?;

        let trajectory = if let Ok(t) =
            tokio::fs::read_to_string(path.join("agent").join("trajectory.json")).await
        {
            serde_json::from_str(&t).ok()
        } else {
            None
        };

        let task = result
            .get("task_name")
            .or_else(|| result.get("trial_name"))
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();

        let execution = result
            .get("agent_execution")
            .cloned()
            .unwrap_or(Value::Null);
        let started_at = execution
            .get("started_at")
            .and_then(Value::as_str)
            .map(String::from);
        let finished_at = execution
            .get("finished_at")
            .and_then(Value::as_str)
            .map(String::from);

        results.push(TrialResult {
            dir_name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            task,
            result,
            trajectory,
            started_at,
            finished_at,
        });
    }
    // Stable order.
    results.sort_by(|a, b| a.task.to_lowercase().cmp(&b.task.to_lowercase()));
    Ok(results)
}

async fn thread_id_for_trial(job_dir: &Path, dir_name: &str) -> Option<String> {
    let coder_txt = job_dir.join(dir_name).join("agent").join("coder.txt");
    let text = tokio::fs::read_to_string(coder_txt)
        .await
        .unwrap_or_default();
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\[oa:thread ([0-9a-fA-F-]{36})\]").unwrap());
    re.captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn print_dry_run_plan(
    suite: &ResolvedSuite,
    model: &str,
    catalog_model: &str,
    lane: &str,
    args: &ExecuteArgs,
) {
    println!(
        "[dry-run] Resolving suite {} ({} tasks)",
        suite.id,
        suite.tasks.len()
    );
    println!("[dry-run] POST /api/v1/gym/runs/start");
    println!(
        "[dry-run]   {}",
        serde_json::json!({
            "suite": suite.id,
            "agent": AGENT_NAME,
            "model": catalog_model,
            "lane": lane,
            "tasks_total": suite.tasks.len(),
        })
    );
    println!(
        "[dry-run]   (will set OPENAGENTS_GYM_RUN_ID=<run-id> and OPENAGENTS_GYM_API_URL=<api-url> for the adapter)"
    );
    let jobs = args
        .jobs_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("/tmp/gym-jobs-<timestamp>"));
    let plan = HostPlan {
        suite: suite.clone(),
        model: model.to_string(),
        lane: lane.to_string(),
        n_concurrent: args.n_concurrent,
        jobs_dir: jobs.clone(),
        timeout_multiplier: args.timeout_multiplier,
        env_provider: args.env.clone(),
        run_id: Some("<run-id>".to_string()),
        api_base: String::new(),
        token: None,
    };
    if will_use_suite_script() {
        println!(
            "[dry-run] preferred: bash bench/run-suite.sh bench/suites/{}.suite.json --model {} --lane {} --jobs-dir {} --n-concurrent {} --api-url <api-url>",
            suite.id,
            model,
            lane,
            jobs.display(),
            args.n_concurrent
        );
    }
    let docker_args = container_docker_args(&plan, &jobs);
    println!("[dry-run] container: docker {}", docker_args.join(" "));
    let host_args = harbor_args(&plan, &jobs);
    println!("[dry-run] host-native: harbor {}", host_args.join(" "));
    println!("[dry-run] POST /api/v1/gym/runs/<run-id>/trials (once per task)");
    println!("[dry-run] PATCH /api/v1/gym/runs/<run-id>");
    println!(
        "[dry-run]   {{\"status\": \"graded\"}} or {{\"status\": \"abandoned\"}} depending on verifier"
    );
}

fn emit_run_status(status: &RunStatus, json: bool) {
    if json {
        match serde_json::to_string(status) {
            Ok(text) => println!("{text}"),
            Err(e) => eprintln!("could not encode run status: {e}"),
        }
    } else {
        crate::gym::views::emit_lines(&crate::gym::views::render_run_status(status));
    }
}

/// Infer the lane from the Harbor model string.
pub fn infer_lane(model: &str) -> String {
    if model.starts_with("ollama/") {
        "local".to_string()
    } else {
        "proxy".to_string()
    }
}

/// Convert a Harbor `provider/name` string to the catalog id the server expects.
pub fn catalog_model(model: &str) -> String {
    let (provider, name) = model.split_once('/').unwrap_or(("", model));
    if provider == "ollama" {
        format!("ollama:{name}")
    } else {
        name.to_string()
    }
}

fn api_to_cli(e: ApiError) -> CliError {
    let (status, code, request_id) = match &e {
        ApiError::Refused {
            status,
            code,
            request_id,
            ..
        } => (*status, code.clone(), request_id.clone()),
        _ => (0, None, None),
    };
    CliError::Api {
        status,
        code,
        message: e.to_string(),
        request_id,
    }
}
