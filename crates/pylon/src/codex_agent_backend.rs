use crate::config::CodexConfig;
use anyhow::Context;
use async_trait::async_trait;
use codex_client::{
    AgentMessageDeltaNotification, AppServerChannels, AppServerClient, AppServerConfig,
    AppServerRequest, AskForApproval, ClientInfo, ErrorNotification, SandboxMode, SandboxPolicy,
    ThreadReadParams, ThreadTokenUsage, ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotification, TurnDiffUpdatedNotification, TurnStartParams, UserInput,
    is_codex_available,
};
use compute::backends::{AgentBackend, AgentCapabilities, AgentError, JobProgress};
use compute::domain::{
    ApprovalStatus, CodeReviewRequest, CodeReviewResult, IssueCategory, IssueSeverity,
    PatchGenRequest, PatchGenResult, PatchVerification, RepoIndexRequest, RepoIndexResult,
    ResourceUsage, ReviewInput, ReviewIssue, SandboxRunRequest, SandboxRunResult, TokenUsage,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::timeout;

const BACKEND_ID: &str = "codex";
const DEFAULT_CODEX_MODEL: &str = "gpt-5.2-codex";
const MAX_GIT_SECONDS: u32 = 300;

pub(crate) struct CodexAgentBackend {
    config: CodexConfig,
    active_jobs: Arc<AtomicU32>,
}

impl CodexAgentBackend {
    pub(crate) fn new(config: CodexConfig) -> Self {
        Self {
            config,
            active_jobs: Arc::new(AtomicU32::new(0)),
        }
    }

    fn select_model(&self, requested: Option<&str>) -> Option<String> {
        let requested = requested
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        if requested.is_some() {
            return requested;
        }

        let configured = self.config.model.trim();
        if configured.is_empty() {
            None
        } else {
            Some(configured.to_string())
        }
    }

    fn model_for_thread_start(model: Option<String>) -> Option<String> {
        model.or_else(|| Some(DEFAULT_CODEX_MODEL.to_string()))
    }

    fn timeout_secs(requested: u32) -> u32 {
        requested.max(30).min(3600)
    }

    fn codex_env(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();
        if let Some(path) = self.config.executable_path.as_ref() {
            env.push((
                "CODEX_APP_SERVER".to_string(),
                path.to_string_lossy().to_string(),
            ));
        }
        env
    }

    async fn spawn_codex_client(
        &self,
        cwd: PathBuf,
    ) -> Result<(AppServerClient, AppServerChannels), AgentError> {
        AppServerClient::spawn(AppServerConfig {
            cwd: Some(cwd),
            wire_log: None,
            env: self.codex_env(),
        })
        .await
        .map_err(|err| AgentError::InitializationError(err.to_string()))
    }

    async fn run_codex_turn(
        &self,
        cwd: &Path,
        model: Option<String>,
        prompt: String,
        time_limit_secs: u32,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<CodexTurnResult, AgentError> {
        let timeout_secs = Self::timeout_secs(time_limit_secs);
        send_progress(
            progress.as_ref(),
            JobProgress::Thinking {
                message: "Starting Codex app-server session".to_string(),
            },
        )
        .await;

        let (client, channels) = self.spawn_codex_client(cwd.to_path_buf()).await?;
        client
            .initialize(ClientInfo {
                name: "pylon-provider".to_string(),
                title: Some("Pylon Provider".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })
            .await
            .map_err(|err| AgentError::InitializationError(err.to_string()))?;

        let thread_model = Self::model_for_thread_start(model.clone());
        let thread_started = client
            .thread_start(codex_client::ThreadStartParams {
                model: thread_model.clone(),
                model_provider: None,
                cwd: Some(cwd.to_string_lossy().to_string()),
                approval_policy: Some(AskForApproval::Never),
                sandbox: Some(SandboxMode::DangerFullAccess),
            })
            .await
            .map_err(|err| AgentError::ExecutionError(err.to_string()))?;

        send_progress(
            progress.as_ref(),
            JobProgress::Thinking {
                message: "Dispatching job to Codex".to_string(),
            },
        )
        .await;

        let turn_started = client
            .turn_start(TurnStartParams {
                thread_id: thread_started.thread.id.clone(),
                input: vec![UserInput::Text { text: prompt }],
                model: model.clone(),
                effort: None,
                summary: None,
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                cwd: Some(cwd.to_string_lossy().to_string()),
            })
            .await
            .map_err(|err| AgentError::ExecutionError(err.to_string()))?;

        let turn_id = turn_started.turn.id.clone();
        let thread_id = thread_started.thread.id.clone();

        let completion = timeout(
            Duration::from_secs(timeout_secs as u64),
            consume_codex_turn(&client, channels, &thread_id, &turn_id, progress.clone()),
        )
        .await;

        let result = match completion {
            Ok(Ok(result)) => result,
            Ok(Err(err)) => {
                let _ = client.shutdown().await;
                return Err(err);
            }
            Err(_) => {
                let _ = client
                    .turn_interrupt(codex_client::TurnInterruptParams {
                        thread_id: thread_id.clone(),
                        turn_id: turn_id.clone(),
                    })
                    .await;
                let _ = client.shutdown().await;
                return Err(AgentError::Timeout(timeout_secs));
            }
        };

        let mut response_text = result.response_text;
        if response_text.trim().is_empty() {
            if let Ok(snapshot) = client
                .thread_read(ThreadReadParams {
                    thread_id: thread_id.clone(),
                    include_turns: true,
                })
                .await
            {
                let fallback = extract_turn_text(&snapshot.thread.turns, &turn_id);
                if !fallback.trim().is_empty() {
                    response_text = fallback;
                }
            }
        }

        if response_text.trim().is_empty()
            && let Some(err) = result.terminal_error.clone()
        {
            let _ = client.shutdown().await;
            return Err(AgentError::ExecutionError(err));
        }

        client
            .shutdown()
            .await
            .map_err(|err| AgentError::ExecutionError(err.to_string()))?;

        Ok(CodexTurnResult {
            response_text,
            model_used: thread_model.unwrap_or_else(|| thread_started.model.clone()),
            usage: result.usage,
            diff: result.diff,
        })
    }

    async fn clone_repo(&self, repo: &str, git_ref: &str) -> Result<ClonedRepo, AgentError> {
        let temp = tempfile::tempdir().map_err(|err| AgentError::IoError(err.to_string()))?;
        let checkout = temp.path().join("repo");
        let checkout_str = checkout.to_string_lossy().to_string();

        let primary = run_process(
            "git",
            &[
                "clone".to_string(),
                "--no-tags".to_string(),
                "--depth".to_string(),
                "1".to_string(),
                "--branch".to_string(),
                git_ref.to_string(),
                repo.to_string(),
                checkout_str.clone(),
            ],
            None,
            Some(&HashMap::from([(
                "GIT_TERMINAL_PROMPT".to_string(),
                "0".to_string(),
            )])),
            MAX_GIT_SECONDS,
        )
        .await;

        if primary.is_err() {
            let fallback = run_process(
                "git",
                &[
                    "clone".to_string(),
                    "--no-tags".to_string(),
                    repo.to_string(),
                    checkout_str.clone(),
                ],
                None,
                Some(&HashMap::from([(
                    "GIT_TERMINAL_PROMPT".to_string(),
                    "0".to_string(),
                )])),
                MAX_GIT_SECONDS,
            )
            .await?;
            if fallback.exit_code != 0 {
                return Err(AgentError::RepositoryError(format!(
                    "git clone failed: {}",
                    fallback.stderr
                )));
            }
            let checkout_result = run_process(
                "git",
                &["checkout".to_string(), git_ref.to_string()],
                Some(&checkout),
                None,
                MAX_GIT_SECONDS,
            )
            .await?;
            if checkout_result.exit_code != 0 {
                return Err(AgentError::RepositoryError(format!(
                    "git checkout failed: {}",
                    checkout_result.stderr
                )));
            }
        } else {
            let primary = primary.expect("checked is_err above");
            if primary.exit_code != 0 {
                return Err(AgentError::RepositoryError(format!(
                    "git clone failed: {}",
                    primary.stderr
                )));
            }
        }

        Ok(ClonedRepo {
            temp,
            path: checkout,
        })
    }
}

#[async_trait]
impl AgentBackend for CodexAgentBackend {
    fn id(&self) -> &str {
        BACKEND_ID
    }

    async fn is_ready(&self) -> bool {
        if !self.config.enabled || !is_codex_available() {
            return false;
        }

        let cwd = self
            .config
            .cwd
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        match self.spawn_codex_client(cwd).await {
            Ok((client, _channels)) => {
                let initialized = client
                    .initialize(ClientInfo {
                        name: "pylon-provider-health".to_string(),
                        title: Some("Pylon Provider Health".to_string()),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    })
                    .await
                    .is_ok();
                let _ = client.shutdown().await;
                initialized
            }
            Err(_) => false,
        }
    }

    fn capabilities(&self) -> AgentCapabilities {
        let mut models = vec![
            "gpt-5.2-codex".to_string(),
            "codex-sonnet-4".to_string(),
            "codex-opus-4".to_string(),
        ];
        let configured_model = self.config.model.trim();
        if !configured_model.is_empty() && !models.iter().any(|model| model == configured_model) {
            models.push(configured_model.to_string());
        }

        AgentCapabilities {
            patch_gen: true,
            code_review: true,
            sandbox_run: true,
            repo_index: false,
            max_concurrent_jobs: 2,
            supported_models: models,
            isolation_mode: "local".to_string(),
            max_time_limit_secs: 3600,
        }
    }

    async fn patch_gen(
        &self,
        request: PatchGenRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> compute::backends::agent::Result<PatchGenResult> {
        let _guard = ActiveJobGuard::new(self.active_jobs.clone());
        let job_started_at = Instant::now();

        send_progress(
            progress.as_ref(),
            JobProgress::CloningRepo {
                repo: request.repo.clone(),
                progress_pct: 10,
            },
        )
        .await;
        let checkout = self.clone_repo(&request.repo, &request.git_ref).await?;

        send_progress(
            progress.as_ref(),
            JobProgress::Thinking {
                message: "Running Codex patch generation".to_string(),
            },
        )
        .await;
        let model = self.select_model(request.model.as_deref());
        let prompt = build_patch_prompt(&request);
        let codex_result = self
            .run_codex_turn(
                &checkout.path,
                model.clone(),
                prompt,
                request.time_limit_secs,
                progress.clone(),
            )
            .await?;

        let patch = collect_patch(&checkout.path, codex_result.diff.as_deref()).await?;
        if patch.trim().is_empty() {
            return Err(AgentError::ExecutionError(
                "Codex produced no patch diff".to_string(),
            ));
        }

        let (files_modified, lines_added, lines_removed) = parse_patch_stats(&patch);
        let mut verification =
            PatchVerification::success(files_modified.len() as u32, lines_added, lines_removed);

        if request.run_tests {
            let test_command = request
                .test_command
                .clone()
                .or_else(|| auto_detect_test_command(&checkout.path));
            if let Some(test_command) = test_command {
                send_progress(
                    progress.as_ref(),
                    JobProgress::Verifying {
                        check: test_command.clone(),
                    },
                )
                .await;
                let test_output = run_shell_command(
                    &checkout.path,
                    &test_command,
                    &HashMap::new(),
                    request.time_limit_secs.min(1200),
                )
                .await?;
                let combined = format!(
                    "$ {}\n{}\n{}",
                    test_command, test_output.stdout, test_output.stderr
                );
                verification = verification.with_test_results(test_output.exit_code, combined);
            }
        }

        let usage = token_usage_from_thread(codex_result.usage);
        let patch_hash = sha256_hex(patch.as_bytes());
        let mut result = PatchGenResult::new(patch, patch_hash)
            .with_verification(verification)
            .with_usage(usage)
            .with_duration(job_started_at.elapsed().as_millis() as u64)
            .with_model(codex_result.model_used)
            .with_summary(extract_summary(&codex_result.response_text));
        for file in files_modified {
            result = result.add_file(file);
        }

        send_progress(
            progress.as_ref(),
            JobProgress::Completed {
                duration_ms: job_started_at.elapsed().as_millis() as u64,
            },
        )
        .await;

        Ok(result)
    }

    async fn code_review(
        &self,
        request: CodeReviewRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> compute::backends::agent::Result<CodeReviewResult> {
        let _guard = ActiveJobGuard::new(self.active_jobs.clone());
        let started_at = Instant::now();

        let checkout = match &request.input {
            ReviewInput::Commits { repo, .. } => Some(self.clone_repo(repo, "HEAD").await?),
            ReviewInput::Files { repo, git_ref, .. } => Some(self.clone_repo(repo, git_ref).await?),
            _ => None,
        };

        let workdir = checkout
            .as_ref()
            .map(|repo| repo.path.clone())
            .or_else(|| self.config.cwd.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        send_progress(
            progress.as_ref(),
            JobProgress::Thinking {
                message: "Running Codex code review".to_string(),
            },
        )
        .await;

        let model = self.select_model(request.model.as_deref());
        let prompt = build_code_review_prompt(&request);
        let codex_result = self
            .run_codex_turn(
                &workdir,
                model,
                prompt,
                request.time_limit_secs,
                progress.clone(),
            )
            .await?;

        let mut review_result =
            if let Some(parsed) = parse_review_response(&codex_result.response_text) {
                parsed.into_result()
            } else {
                let text = codex_result.response_text.trim();
                let summary = if text.is_empty() {
                    "Codex completed review with no textual output.".to_string()
                } else {
                    text.to_string()
                };
                CodeReviewResult::comment(summary)
            };

        if review_result.review_sha256.is_empty() {
            review_result =
                review_result.with_hash(sha256_hex(codex_result.response_text.as_bytes()));
        }
        if let Some(usage) = codex_result.usage.as_ref() {
            let input_tokens = usage
                .input_tokens
                .saturating_add(usage.cached_input_tokens)
                .max(0) as u64;
            let output_tokens = usage.output_tokens.max(0) as u64;
            review_result = review_result.with_tokens(input_tokens, output_tokens);
        }
        review_result = review_result
            .with_duration(started_at.elapsed().as_millis() as u64)
            .with_model(codex_result.model_used);

        send_progress(
            progress.as_ref(),
            JobProgress::Completed {
                duration_ms: started_at.elapsed().as_millis() as u64,
            },
        )
        .await;

        Ok(review_result)
    }

    async fn sandbox_run(
        &self,
        request: SandboxRunRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> compute::backends::agent::Result<SandboxRunResult> {
        let _guard = ActiveJobGuard::new(self.active_jobs.clone());
        let started_at = Instant::now();
        send_progress(
            progress.as_ref(),
            JobProgress::CloningRepo {
                repo: request.repo.clone(),
                progress_pct: 10,
            },
        )
        .await;
        let checkout = self.clone_repo(&request.repo, &request.git_ref).await?;

        let workdir = if let Some(relative) = request.workdir.as_ref() {
            checkout.path.join(relative)
        } else {
            checkout.path.clone()
        };
        if !workdir.exists() {
            return Err(AgentError::InvalidRequest(format!(
                "sandbox workdir does not exist: {}",
                workdir.display()
            )));
        }

        let mut overall_exit = 0;
        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();
        let mut command_results = Vec::new();
        let mut per_command_timeout = request.limits.max_time_secs.max(10);

        if request.commands.is_empty() {
            return Err(AgentError::InvalidRequest(
                "sandbox run requires at least one command".to_string(),
            ));
        }
        if per_command_timeout > 3600 {
            per_command_timeout = 3600;
        }

        for command in &request.commands {
            send_progress(
                progress.as_ref(),
                JobProgress::ToolUse {
                    tool: "shell".to_string(),
                    input_preview: truncate(command, 120),
                },
            )
            .await;

            let run =
                run_shell_command(&workdir, command, &request.env, per_command_timeout).await?;
            let command_result = compute::domain::CommandResult {
                command: command.clone(),
                exit_code: run.exit_code,
                stdout: run.stdout.clone(),
                stderr: run.stderr.clone(),
                duration_ms: run.duration_ms,
            };
            command_results.push(command_result);
            if !run.stdout.is_empty() {
                if !combined_stdout.is_empty() {
                    combined_stdout.push('\n');
                }
                combined_stdout.push_str(&run.stdout);
            }
            if !run.stderr.is_empty() {
                if !combined_stderr.is_empty() {
                    combined_stderr.push('\n');
                }
                combined_stderr.push_str(&run.stderr);
            }

            send_progress(
                progress.as_ref(),
                JobProgress::ToolResult {
                    tool: "shell".to_string(),
                    success: run.exit_code == 0,
                    output_preview: Some(truncate(&run.stdout, 120)),
                },
            )
            .await;

            if run.exit_code != 0 {
                overall_exit = run.exit_code;
                break;
            }
        }

        let artifacts = collect_changed_artifacts(&checkout.path).await?;
        let mut result = SandboxRunResult::new(overall_exit);
        result.stdout = combined_stdout;
        result.stderr = combined_stderr;
        result.command_results = command_results;
        result.artifacts = artifacts;
        result.usage = ResourceUsage {
            cpu_time_ms: started_at.elapsed().as_millis() as u64,
            peak_memory_bytes: 0,
            disk_writes_bytes: 0,
            network_bytes: 0,
        };

        send_progress(
            progress.as_ref(),
            JobProgress::Completed {
                duration_ms: started_at.elapsed().as_millis() as u64,
            },
        )
        .await;

        Ok(result)
    }

    async fn repo_index(
        &self,
        _request: RepoIndexRequest,
        _progress: Option<mpsc::Sender<JobProgress>>,
    ) -> compute::backends::agent::Result<RepoIndexResult> {
        Err(AgentError::InvalidRequest(
            "Codex backend does not support repo_index jobs yet".to_string(),
        ))
    }

    async fn cancel(&self, _job_id: &str) -> compute::backends::agent::Result<()> {
        Err(AgentError::Cancelled(
            "Codex backend does not support external cancellation".to_string(),
        ))
    }

    fn active_jobs(&self) -> u32 {
        self.active_jobs.load(Ordering::Relaxed)
    }
}

struct ActiveJobGuard {
    counter: Arc<AtomicU32>,
}

impl ActiveJobGuard {
    fn new(counter: Arc<AtomicU32>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for ActiveJobGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Debug)]
struct ClonedRepo {
    #[allow(dead_code)]
    temp: TempDir,
    path: PathBuf,
}

#[derive(Debug)]
struct CodexTurnResult {
    response_text: String,
    model_used: String,
    usage: Option<ThreadTokenUsage>,
    diff: Option<String>,
}

#[derive(Debug, Default)]
struct TurnConsumerResult {
    response_text: String,
    usage: Option<ThreadTokenUsage>,
    diff: Option<String>,
    terminal_error: Option<String>,
}

#[derive(Debug)]
struct ProcessResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

async fn consume_codex_turn(
    client: &AppServerClient,
    channels: AppServerChannels,
    thread_id: &str,
    turn_id: &str,
    progress: Option<mpsc::Sender<JobProgress>>,
) -> Result<TurnConsumerResult, AgentError> {
    let mut notifications = channels.notifications;
    let mut requests = channels.requests;
    let mut result = TurnConsumerResult::default();

    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else {
                    break;
                };
                let response = auto_response_for_request(&request);
                client
                    .respond(request.id, &response)
                    .await
                    .map_err(|err| AgentError::ExecutionError(err.to_string()))?;
            }
            notification = notifications.recv() => {
                let Some(notification) = notification else {
                    break;
                };
                match notification.method.as_str() {
                    "item/agentMessage/delta" => {
                        if let Some(params) = notification.params
                            && let Ok(event) = serde_json::from_value::<AgentMessageDeltaNotification>(params)
                            && event.thread_id == thread_id
                            && event.turn_id == turn_id
                        {
                            result.response_text.push_str(&event.delta);
                        }
                    }
                    "thread/tokenUsage/updated" => {
                        if let Some(params) = notification.params
                            && let Ok(event) = serde_json::from_value::<ThreadTokenUsageUpdatedNotification>(params)
                            && event.thread_id == thread_id
                            && event.turn_id == turn_id
                        {
                            result.usage = Some(event.token_usage);
                        }
                    }
                    "turn/diff/updated" => {
                        if let Some(params) = notification.params
                            && let Ok(event) = serde_json::from_value::<TurnDiffUpdatedNotification>(params)
                            && event.thread_id == thread_id
                            && event.turn_id == turn_id
                        {
                            result.diff = Some(event.diff);
                        }
                    }
                    "error" => {
                        if let Some(params) = notification.params
                            && let Ok(error) = serde_json::from_value::<ErrorNotification>(params)
                            && error.thread_id == thread_id
                            && error.turn_id == turn_id
                            && !error.will_retry
                        {
                            result.terminal_error = Some(error.error.message);
                            break;
                        }
                    }
                    "turn/completed" => {
                        if let Some(params) = notification.params
                            && let Ok(event) = serde_json::from_value::<TurnCompletedNotification>(params)
                            && event.thread_id == thread_id
                            && event.turn.id == turn_id
                        {
                            break;
                        }
                    }
                    "item/started" => {
                        if let Some(progress) = progress.as_ref() {
                            let _ = progress
                                .send(JobProgress::Thinking {
                                    message: "Codex is processing job steps".to_string(),
                                })
                                .await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(result)
}

fn auto_response_for_request(request: &AppServerRequest) -> Value {
    match request.method.as_str() {
        "execCommandApproval" | "applyPatchApproval" => json!({ "decision": "approved" }),
        "item/tool/requestUserInput" => request
            .params
            .as_ref()
            .map(build_tool_input_response)
            .unwrap_or_else(|| json!({"answers": {}})),
        _ => json!({}),
    }
}

fn build_tool_input_response(params: &Value) -> Value {
    let mut answers = serde_json::Map::new();
    let questions = params
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for question in questions {
        let id = question
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let answer = question
            .get("options")
            .and_then(Value::as_array)
            .and_then(|options| options.first())
            .and_then(|option| option.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "yes".to_string());
        if let Some(id) = id {
            answers.insert(
                id,
                json!({
                    "answers": [answer],
                }),
            );
        }
    }

    json!({ "answers": answers })
}

fn build_patch_prompt(request: &PatchGenRequest) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are servicing a NIP-90 PatchGen compute job.\n");
    prompt.push_str("Apply code changes directly in the checked out repository.\n\n");
    prompt.push_str("Task:\n");
    prompt.push_str(request.issue.trim());
    prompt.push('\n');

    if let Some(issue_url) = request.issue_url.as_deref()
        && !issue_url.trim().is_empty()
    {
        prompt.push_str("\nIssue URL:\n");
        prompt.push_str(issue_url.trim());
        prompt.push('\n');
    }
    if let Some(context) = request.context.as_deref()
        && !context.trim().is_empty()
    {
        prompt.push_str("\nAdditional context:\n");
        prompt.push_str(context.trim());
        prompt.push('\n');
    }
    if !request.path_filter.include.is_empty() {
        prompt.push_str("\nAllowed include globs:\n");
        for pattern in &request.path_filter.include {
            prompt.push_str("- ");
            prompt.push_str(pattern);
            prompt.push('\n');
        }
    }
    if !request.path_filter.exclude.is_empty() {
        prompt.push_str("\nDisallowed exclude globs:\n");
        for pattern in &request.path_filter.exclude {
            prompt.push_str("- ");
            prompt.push_str(pattern);
            prompt.push('\n');
        }
    }
    if request.run_tests {
        prompt.push_str("\nRun tests after patching.");
        if let Some(command) = request.test_command.as_deref() {
            prompt.push_str("\nPreferred test command: ");
            prompt.push_str(command.trim());
        }
        prompt.push('\n');
    }
    prompt.push_str("\nReturn a concise summary of what changed and why.\n");
    prompt
}

fn build_code_review_prompt(request: &CodeReviewRequest) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are servicing a NIP-90 CodeReview compute job.\n");
    prompt.push_str("Review the provided change and return STRICT JSON only.\n\n");
    prompt.push_str("JSON schema:\n");
    prompt.push_str(
        "{\n  \"status\": \"approve|request_changes|comment\",\n  \"summary\": \"string\",\n  \"issues\": [\n    {\n      \"file_path\": \"string\",\n      \"line\": 1,\n      \"end_line\": 1,\n      \"severity\": \"critical|major|minor|nit\",\n      \"category\": \"security|bug|performance|style|documentation|testing|architecture|maintainability|other\",\n      \"title\": \"string\",\n      \"description\": \"string\",\n      \"suggestion\": \"string\"\n    }\n  ]\n}\n\n",
    );
    prompt.push_str("Input:\n");
    match &request.input {
        ReviewInput::Diff(diff) => {
            prompt.push_str("Type: diff\n");
            prompt.push_str(diff);
            prompt.push('\n');
        }
        ReviewInput::PullRequest { url } => {
            prompt.push_str("Type: pull_request\n");
            prompt.push_str("URL: ");
            prompt.push_str(url);
            prompt.push('\n');
        }
        ReviewInput::Commits { repo, shas } => {
            prompt.push_str("Type: commits\n");
            prompt.push_str("Repo: ");
            prompt.push_str(repo);
            prompt.push('\n');
            prompt.push_str("SHAs:\n");
            for sha in shas {
                prompt.push_str("- ");
                prompt.push_str(sha);
                prompt.push('\n');
            }
        }
        ReviewInput::Files {
            repo,
            git_ref,
            paths,
        } => {
            prompt.push_str("Type: files\n");
            prompt.push_str("Repo: ");
            prompt.push_str(repo);
            prompt.push('\n');
            prompt.push_str("Ref: ");
            prompt.push_str(git_ref);
            prompt.push('\n');
            prompt.push_str("Paths:\n");
            for path in paths {
                prompt.push_str("- ");
                prompt.push_str(path);
                prompt.push('\n');
            }
        }
    }

    if !request.focus_areas.is_empty() {
        prompt.push_str("\nFocus areas:\n");
        for area in &request.focus_areas {
            prompt.push_str("- ");
            prompt.push_str(area.as_str());
            prompt.push('\n');
        }
    }
    if let Some(guidelines) = request.guidelines.as_deref()
        && !guidelines.trim().is_empty()
    {
        prompt.push_str("\nGuidelines:\n");
        prompt.push_str(guidelines.trim());
        prompt.push('\n');
    }
    if request.include_nits {
        prompt.push_str("\nInclude style nits if relevant.\n");
    }
    prompt
}

async fn collect_patch(
    repo_path: &Path,
    diff_from_notifications: Option<&str>,
) -> Result<String, AgentError> {
    let _ = run_process(
        "git",
        &["add".to_string(), "-N".to_string(), ".".to_string()],
        Some(repo_path),
        None,
        30,
    )
    .await;
    let diff = run_process(
        "git",
        &["diff".to_string(), "--no-color".to_string()],
        Some(repo_path),
        None,
        60,
    )
    .await?;
    if !diff.stdout.trim().is_empty() {
        return Ok(diff.stdout);
    }
    Ok(diff_from_notifications.unwrap_or_default().to_string())
}

fn parse_patch_stats(patch: &str) -> (Vec<String>, u32, u32) {
    let mut files = HashSet::new();
    let mut lines_added = 0u32;
    let mut lines_removed = 0u32;

    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            if let Some(path) = line.split_whitespace().nth(3) {
                let file = path.trim_start_matches("b/").to_string();
                files.insert(file);
            }
            continue;
        }
        if line.starts_with("+++ ") || line.starts_with("--- ") {
            continue;
        }
        if line.starts_with('+') {
            lines_added = lines_added.saturating_add(1);
        } else if line.starts_with('-') {
            lines_removed = lines_removed.saturating_add(1);
        }
    }

    let mut files_vec: Vec<String> = files.into_iter().collect();
    files_vec.sort();
    (files_vec, lines_added, lines_removed)
}

fn token_usage_from_thread(usage: Option<ThreadTokenUsage>) -> TokenUsage {
    if let Some(usage) = usage {
        TokenUsage {
            input_tokens: usage
                .input_tokens
                .saturating_add(usage.cached_input_tokens)
                .max(0) as u64,
            output_tokens: usage.output_tokens.max(0) as u64,
            cache_read_tokens: usage.cached_input_tokens.max(0) as u64,
            cache_write_tokens: 0,
        }
    } else {
        TokenUsage::default()
    }
}

fn auto_detect_test_command(repo_path: &Path) -> Option<String> {
    if repo_path.join("Cargo.toml").exists() {
        return Some("cargo test --all-targets".to_string());
    }
    if repo_path.join("package.json").exists() {
        return Some("npm test".to_string());
    }
    if repo_path.join("pyproject.toml").exists() || repo_path.join("pytest.ini").exists() {
        return Some("pytest".to_string());
    }
    None
}

async fn run_shell_command(
    cwd: &Path,
    command: &str,
    env: &HashMap<String, String>,
    timeout_secs: u32,
) -> Result<ProcessResult, AgentError> {
    run_process(
        "/bin/sh",
        &["-lc".to_string(), command.to_string()],
        Some(cwd),
        Some(env),
        timeout_secs,
    )
    .await
}

async fn run_process(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    env: Option<&HashMap<String, String>>,
    timeout_secs: u32,
) -> Result<ProcessResult, AgentError> {
    let mut command = Command::new(program);
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    if let Some(env) = env {
        command.envs(env);
    }
    let started_at = Instant::now();
    let output = timeout(
        Duration::from_secs(timeout_secs.max(1) as u64),
        command.output(),
    )
    .await
    .map_err(|_| AgentError::Timeout(timeout_secs.max(1)))?
    .map_err(|err| AgentError::ExecutionError(err.to_string()))?;

    Ok(ProcessResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms: started_at.elapsed().as_millis() as u64,
    })
}

async fn collect_changed_artifacts(
    repo_path: &Path,
) -> Result<Vec<compute::domain::ArtifactHash>, AgentError> {
    let status = run_process(
        "git",
        &["status".to_string(), "--porcelain".to_string()],
        Some(repo_path),
        None,
        30,
    )
    .await?;
    if status.exit_code != 0 {
        return Ok(Vec::new());
    }

    let mut artifacts = Vec::new();
    for line in status.stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let path = line[3..].trim();
        if path.is_empty() {
            continue;
        }
        let full_path = repo_path.join(path);
        if !full_path.exists() || full_path.is_dir() {
            continue;
        }
        let bytes = tokio::fs::read(&full_path)
            .await
            .with_context(|| format!("read artifact {}", full_path.display()))
            .map_err(|err| AgentError::IoError(err.to_string()))?;
        let metadata = tokio::fs::metadata(&full_path)
            .await
            .map_err(|err| AgentError::IoError(err.to_string()))?;
        artifacts.push(compute::domain::ArtifactHash {
            path: path.to_string(),
            sha256: sha256_hex(&bytes),
            size: metadata.len(),
        });
    }

    Ok(artifacts)
}

fn extract_summary(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Codex completed the patch job.".to_string();
    }
    truncate(trimmed, 320)
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let truncated = text.chars().take(max_chars).collect::<String>();
    format!("{truncated}...")
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn extract_turn_text(turns: &[codex_client::ThreadTurn], turn_id: &str) -> String {
    let Some(turn) = turns.iter().find(|turn| turn.id == turn_id) else {
        return String::new();
    };
    let mut chunks = Vec::new();
    for item in &turn.items {
        collect_text_candidates(item, &mut chunks);
    }
    chunks.join("")
}

fn collect_text_candidates(value: &Value, chunks: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if !text.trim().is_empty() {
                chunks.push(text.clone());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_text_candidates(item, chunks);
            }
        }
        Value::Object(map) => {
            for key in ["text", "delta", "content", "message", "summary", "output"] {
                if let Some(value) = map.get(key) {
                    collect_text_candidates(value, chunks);
                }
            }
        }
        _ => {}
    }
}

#[derive(Debug, Deserialize)]
struct ParsedReviewResponse {
    status: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    issues: Vec<ParsedReviewIssue>,
}

#[derive(Debug, Deserialize)]
struct ParsedReviewIssue {
    file_path: Option<String>,
    line: Option<u32>,
    end_line: Option<u32>,
    severity: Option<String>,
    category: Option<String>,
    title: Option<String>,
    description: Option<String>,
    suggestion: Option<String>,
    code_snippet: Option<String>,
}

impl ParsedReviewResponse {
    fn into_result(self) -> CodeReviewResult {
        let summary = self
            .summary
            .unwrap_or_else(|| "Code review completed.".to_string());
        let status = self
            .status
            .as_deref()
            .and_then(ApprovalStatus::from_str)
            .unwrap_or_else(|| {
                if self.issues.is_empty() {
                    ApprovalStatus::Approve
                } else {
                    ApprovalStatus::RequestChanges
                }
            });

        let mut result = match status {
            ApprovalStatus::Approve => CodeReviewResult::approve(summary),
            ApprovalStatus::RequestChanges => CodeReviewResult::request_changes(summary),
            ApprovalStatus::Comment => CodeReviewResult::comment(summary),
        };

        for issue in self.issues {
            let severity = issue
                .severity
                .as_deref()
                .and_then(IssueSeverity::from_str)
                .unwrap_or(IssueSeverity::Minor);
            let category = issue
                .category
                .as_deref()
                .and_then(IssueCategory::from_str)
                .unwrap_or(IssueCategory::Other);
            let mut review_issue = ReviewIssue::new(
                issue
                    .file_path
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                severity,
                category,
                issue
                    .title
                    .clone()
                    .unwrap_or_else(|| "Review issue".to_string()),
                issue
                    .description
                    .clone()
                    .unwrap_or_else(|| "Details unavailable".to_string()),
            );
            if let Some(line) = issue.line {
                if let Some(end_line) = issue.end_line {
                    review_issue = review_issue.at_lines(line, end_line);
                } else {
                    review_issue = review_issue.at_line(line);
                }
            }
            if let Some(suggestion) = issue.suggestion.as_deref()
                && !suggestion.trim().is_empty()
            {
                review_issue = review_issue.with_suggestion(suggestion);
            }
            if let Some(snippet) = issue.code_snippet.as_deref()
                && !snippet.trim().is_empty()
            {
                review_issue = review_issue.with_snippet(snippet);
            }
            result = result.add_issue(review_issue);
        }

        result
    }
}

fn parse_review_response(response: &str) -> Option<ParsedReviewResponse> {
    if let Ok(parsed) = serde_json::from_str::<ParsedReviewResponse>(response) {
        return Some(parsed);
    }

    if let Some(block) = extract_markdown_json_block(response)
        && let Ok(parsed) = serde_json::from_str::<ParsedReviewResponse>(&block)
    {
        return Some(parsed);
    }

    if let Some(object) = extract_json_object(response)
        && let Ok(parsed) = serde_json::from_str::<ParsedReviewResponse>(&object)
    {
        return Some(parsed);
    }

    None
}

fn extract_markdown_json_block(text: &str) -> Option<String> {
    let marker = "```json";
    let start = text.find(marker)?;
    let rest = &text[start + marker.len()..];
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(text[start..=end].to_string())
}

async fn send_progress(progress: Option<&mpsc::Sender<JobProgress>>, event: JobProgress) {
    if let Some(progress) = progress {
        let _ = progress.send(event).await;
    }
}
