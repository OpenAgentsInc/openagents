use std::collections::{HashMap, HashSet};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, ValueEnum};
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, AppServerNotification, AppServerRequest,
    ApprovalDecision, AskForApproval, ClientInfo, CommandExecutionRequestApprovalResponse,
    DynamicToolCallResponse, FileChangeRequestApprovalResponse, InitializeCapabilities,
    InitializeParams, Personality, SandboxMode, SandboxPolicy, TextElement, ThreadResumeParams,
    ThreadStartParams, ToolRequestUserInputResponse, TurnInterruptParams, TurnStartParams,
    UserInput,
};
use serde::Serialize;
use serde_json::{Map, Value};
use tokio::time::timeout;

const DRAIN_TIMEOUT: Duration = Duration::from_millis(40);

#[derive(Parser, Debug)]
#[command(name = "autopilot-codex-exec", version)]
pub struct Cli {
    /// Optional image(s) to attach to the prompt.
    #[arg(
        long = "image",
        short = 'i',
        value_name = "FILE",
        value_delimiter = ',',
        num_args = 1..
    )]
    images: Vec<PathBuf>,

    /// Model the agent should use.
    #[arg(long, short = 'm')]
    model: Option<String>,

    /// Tell the agent to use the specified directory as its working root.
    #[arg(long = "cd", short = 'C', value_name = "DIR")]
    cwd: Option<PathBuf>,

    /// Resume a previously created thread instead of starting a new one.
    #[arg(long = "thread-id", value_name = "THREAD_ID")]
    thread_id: Option<String>,

    /// Non-interactive approval policy for the run.
    #[arg(long = "approval-policy", value_enum)]
    approval_policy: Option<ApprovalPolicyArg>,

    /// Sandbox mode for model-generated shell commands.
    #[arg(long = "sandbox", short = 's', value_enum)]
    sandbox_mode: Option<SandboxModeArg>,

    /// Convenience alias for automatic sandboxed execution.
    #[arg(long = "full-auto", default_value_t = false)]
    full_auto: bool,

    /// Skip approvals and run without sandboxing.
    #[arg(
        long = "dangerously-bypass-approvals-and-sandbox",
        alias = "yolo",
        default_value_t = false,
        conflicts_with = "full_auto"
    )]
    dangerously_bypass_approvals_and_sandbox: bool,

    /// Run without persisting the session to disk.
    #[arg(long = "ephemeral", default_value_t = false)]
    ephemeral: bool,

    /// Path to a JSON Schema file describing the final response shape.
    #[arg(long = "output-schema", value_name = "FILE")]
    output_schema: Option<PathBuf>,

    /// Path where the final assistant message should be written.
    #[arg(long = "output-last-message", short = 'o', value_name = "FILE")]
    last_message_file: Option<PathBuf>,

    /// Print machine-readable events to stdout as JSONL.
    #[arg(long = "json", alias = "experimental-json", default_value_t = false)]
    json: bool,

    /// Optional app-server wire log path for debugging automation runs.
    #[arg(long = "wire-log", value_name = "FILE")]
    wire_log: Option<PathBuf>,

    /// Initial prompt. When omitted or `-`, read from stdin.
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum ApprovalPolicyArg {
    Never,
    OnRequest,
    OnFailure,
    UnlessTrusted,
}

impl From<ApprovalPolicyArg> for AskForApproval {
    fn from(value: ApprovalPolicyArg) -> Self {
        match value {
            ApprovalPolicyArg::Never => Self::Never,
            ApprovalPolicyArg::OnRequest => Self::OnRequest,
            ApprovalPolicyArg::OnFailure => Self::OnFailure,
            ApprovalPolicyArg::UnlessTrusted => Self::UnlessTrusted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum SandboxModeArg {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl From<SandboxModeArg> for SandboxMode {
    fn from(value: SandboxModeArg) -> Self {
        match value {
            SandboxModeArg::ReadOnly => Self::ReadOnly,
            SandboxModeArg::WorkspaceWrite => Self::WorkspaceWrite,
            SandboxModeArg::DangerFullAccess => Self::DangerFullAccess,
        }
    }
}

#[derive(Debug, Clone)]
struct RunRequest {
    prompt: String,
    images: Vec<PathBuf>,
    model: Option<String>,
    cwd: Option<PathBuf>,
    resume_thread_id: Option<String>,
    approval_policy: AskForApproval,
    sandbox_mode: Option<SandboxMode>,
    ephemeral: bool,
    output_schema: Option<Value>,
    last_message_file: Option<PathBuf>,
    json_mode: bool,
    wire_log: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
struct Usage {
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct ExecError {
    message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct ExecItem {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aggregated_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<String>,
    raw: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type")]
enum ExecEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },
    #[serde(rename = "turn.started")]
    TurnStarted { thread_id: String, turn_id: String },
    #[serde(rename = "turn.completed")]
    TurnCompleted {
        thread_id: String,
        turn_id: String,
        usage: Usage,
    },
    #[serde(rename = "turn.failed")]
    TurnFailed {
        thread_id: String,
        turn_id: String,
        error: ExecError,
    },
    #[serde(rename = "item.started")]
    ItemStarted {
        thread_id: String,
        turn_id: String,
        item: ExecItem,
    },
    #[serde(rename = "item.updated")]
    ItemUpdated {
        thread_id: String,
        turn_id: String,
        item: ExecItem,
    },
    #[serde(rename = "item.completed")]
    ItemCompleted {
        thread_id: String,
        turn_id: String,
        item: ExecItem,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
        message: String,
    },
}

#[derive(Default)]
struct PendingTextItem {
    kind: &'static str,
    text: String,
}

#[derive(Default)]
struct ExecEventProjector {
    buffered_text: HashMap<String, PendingTextItem>,
    buffered_command_output: HashMap<String, String>,
    usage_by_turn: HashMap<String, Usage>,
    completed_item_ids: HashSet<String>,
    final_message: Option<String>,
}

struct TurnOutcome {
    final_message: Option<String>,
}

pub fn main_entry() -> Result<()> {
    let cli = Cli::parse();
    let request = build_run_request(cli)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build runtime")?;
    let mut stdout = io::stdout().lock();
    runtime.block_on(run_with_spawned_client(request, &mut stdout))?;
    Ok(())
}

fn build_run_request(cli: Cli) -> Result<RunRequest> {
    if cli.ephemeral && cli.thread_id.is_some() {
        bail!("--ephemeral cannot be combined with --thread-id");
    }

    let prompt = resolve_prompt(cli.prompt.as_deref())?;
    if prompt.trim().is_empty() && cli.images.is_empty() {
        bail!("a prompt or at least one --image is required");
    }

    let output_schema = cli
        .output_schema
        .as_ref()
        .map(|path| load_output_schema(path.as_path()))
        .transpose()?;

    let sandbox_mode = if cli.dangerously_bypass_approvals_and_sandbox {
        Some(SandboxMode::DangerFullAccess)
    } else if let Some(mode) = cli.sandbox_mode {
        Some(mode.into())
    } else if cli.full_auto {
        Some(SandboxMode::WorkspaceWrite)
    } else {
        None
    };

    let approval_policy = cli
        .approval_policy
        .map(Into::into)
        .unwrap_or(AskForApproval::Never);

    Ok(RunRequest {
        prompt,
        images: cli.images,
        model: cli.model,
        cwd: cli.cwd,
        resume_thread_id: cli.thread_id,
        approval_policy,
        sandbox_mode,
        ephemeral: cli.ephemeral,
        output_schema,
        last_message_file: cli.last_message_file,
        json_mode: cli.json,
        wire_log: cli.wire_log,
    })
}

fn resolve_prompt(prompt: Option<&str>) -> Result<String> {
    match prompt {
        Some(value) if value != "-" => Ok(value.to_string()),
        _ => {
            let mut buffer = String::new();
            if io::stdin().is_terminal() {
                return Ok(buffer);
            }
            io::stdin()
                .read_to_string(&mut buffer)
                .context("failed to read stdin prompt")?;
            Ok(buffer)
        }
    }
}

fn load_output_schema(path: &Path) -> Result<Value> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read output schema {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse output schema {}", path.display()))
}

async fn run_with_spawned_client<W: Write>(request: RunRequest, stdout: &mut W) -> Result<()> {
    let wire_log = request.wire_log.as_ref().map(|path| {
        let wire_log = codex_client::AppServerWireLog::new();
        wire_log.set_path(path.clone());
        wire_log
    });
    let (client, channels) = AppServerClient::spawn(AppServerConfig {
        cwd: request.cwd.clone(),
        wire_log,
        env: Vec::new(),
    })
    .await
    .context("failed to spawn codex app-server")?;
    run_with_client(client, channels, request, stdout).await
}

async fn run_with_client<W: Write>(
    client: AppServerClient,
    channels: AppServerChannels,
    request: RunRequest,
    stdout: &mut W,
) -> Result<()> {
    let result = run_session(client, channels, &request, stdout).await;
    if let Some(path) = &request.last_message_file {
        let final_message = match &result {
            Ok(outcome) => outcome.final_message.as_deref().unwrap_or_default(),
            Err(_) => "",
        };
        std::fs::write(path, final_message)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    result.map(|_| ())
}

async fn run_session<W: Write>(
    client: AppServerClient,
    mut channels: AppServerChannels,
    request: &RunRequest,
    stdout: &mut W,
) -> Result<TurnOutcome> {
    client
        .initialize(InitializeParams {
            client_info: ClientInfo {
                name: "openagents-autopilot-codex-exec".to_string(),
                title: Some("OpenAgents Autopilot Codex Exec".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                opt_out_notification_methods: Some(
                    codex_client::legacy_codex_event_opt_out_notification_methods()
                        .iter()
                        .map(|method| (*method).to_string())
                        .collect(),
                ),
            }),
        })
        .await
        .context("initialize failed")?;

    let thread_id = if let Some(thread_id) = &request.resume_thread_id {
        let response = client
            .thread_resume(ThreadResumeParams {
                thread_id: thread_id.clone(),
                path: None,
                model: request.model.clone(),
                model_provider: None,
                service_tier: None,
                cwd: request.cwd.as_ref().map(|path| path.display().to_string()),
                approval_policy: Some(request.approval_policy),
                sandbox: request.sandbox_mode,
                personality: Some(Personality::Pragmatic),
            })
            .await
            .with_context(|| format!("thread/resume failed for {}", thread_id))?;
        response.thread.id
    } else {
        let response = client
            .thread_start(ThreadStartParams {
                model: request.model.clone(),
                model_provider: None,
                service_tier: None,
                cwd: request.cwd.as_ref().map(|path| path.display().to_string()),
                approval_policy: Some(request.approval_policy),
                sandbox: request.sandbox_mode,
                personality: Some(Personality::Pragmatic),
                ephemeral: Some(request.ephemeral),
                dynamic_tools: None,
            })
            .await
            .context("thread/start failed")?;
        response.thread.id
    };
    emit_event(
        stdout,
        request.json_mode,
        &ExecEvent::ThreadStarted {
            thread_id: thread_id.clone(),
        },
    )?;

    let input = assemble_turn_input(request);
    let turn = client
        .turn_start(TurnStartParams {
            thread_id: thread_id.clone(),
            input,
            cwd: request.cwd.clone(),
            approval_policy: Some(request.approval_policy),
            sandbox_policy: sandbox_policy_from_mode(request.sandbox_mode),
            model: request.model.clone(),
            service_tier: None,
            effort: None,
            summary: None,
            personality: Some(Personality::Pragmatic),
            output_schema: request.output_schema.clone(),
            collaboration_mode: None,
        })
        .await
        .context("turn/start failed")?;
    let turn_id = turn.turn.id;
    emit_event(
        stdout,
        request.json_mode,
        &ExecEvent::TurnStarted {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
        },
    )?;

    let result = run_turn_loop(
        &client,
        &mut channels,
        stdout,
        request.json_mode,
        thread_id.clone(),
        turn_id.clone(),
    )
    .await;

    let shutdown_result = client.shutdown().await;
    match (result, shutdown_result) {
        (Ok(outcome), Ok(())) => {
            if !request.json_mode
                && let Some(final_message) = outcome.final_message.as_deref()
                && !final_message.is_empty()
            {
                stdout
                    .write_all(final_message.as_bytes())
                    .context("failed to write final message")?;
                if !final_message.ends_with('\n') {
                    stdout
                        .write_all(b"\n")
                        .context("failed to write final newline")?;
                }
            }
            Ok(outcome)
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error.context("failed to shutdown app-server client")),
        (Err(primary), Err(_)) => Err(primary),
    }
}

fn assemble_turn_input(request: &RunRequest) -> Vec<UserInput> {
    let mut input = Vec::new();
    if !request.prompt.is_empty() {
        input.push(UserInput::Text {
            text: request.prompt.clone(),
            text_elements: Vec::<TextElement>::new(),
        });
    }
    input.extend(
        request
            .images
            .iter()
            .cloned()
            .map(|path| UserInput::LocalImage { path }),
    );
    input
}

fn sandbox_policy_from_mode(mode: Option<SandboxMode>) -> Option<SandboxPolicy> {
    match mode {
        Some(SandboxMode::ReadOnly) => Some(SandboxPolicy::ReadOnly),
        Some(SandboxMode::WorkspaceWrite) => Some(SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }),
        Some(SandboxMode::DangerFullAccess) => Some(SandboxPolicy::DangerFullAccess),
        None => None,
    }
}

async fn run_turn_loop<W: Write>(
    client: &AppServerClient,
    channels: &mut AppServerChannels,
    stdout: &mut W,
    json_mode: bool,
    thread_id: String,
    turn_id: String,
) -> Result<TurnOutcome> {
    let mut projector = ExecEventProjector::default();
    let mut turn_failure: Option<String> = None;
    let mut turn_finished = false;

    while !turn_finished && turn_failure.is_none() {
        tokio::select! {
            maybe_notification = channels.notifications.recv() => {
                let Some(notification) = maybe_notification else {
                    break;
                };
                match projector.handle_notification(stdout, json_mode, &thread_id, &turn_id, notification)? {
                    NotificationFlow::Continue => {}
                    NotificationFlow::Completed => turn_finished = true,
                    NotificationFlow::Failed(message) => turn_failure = Some(message),
                }
            }
            maybe_request = channels.requests.recv() => {
                let Some(request) = maybe_request else {
                    break;
                };
                if let Some(message) = handle_server_request(client, request).await? {
                    emit_event(
                        stdout,
                        json_mode,
                        &ExecEvent::Error {
                            thread_id: Some(thread_id.clone()),
                            turn_id: Some(turn_id.clone()),
                            message: message.clone(),
                        },
                    )?;
                    let _ = client.turn_interrupt(TurnInterruptParams {
                        thread_id: thread_id.clone(),
                        turn_id: turn_id.clone(),
                    }).await;
                    turn_failure = Some(message);
                }
            }
        }
    }

    drain_post_turn_notifications(
        channels,
        &mut projector,
        stdout,
        json_mode,
        &thread_id,
        &turn_id,
    )
    .await?;

    projector.flush_pending_items(stdout, json_mode, &thread_id, &turn_id)?;

    if let Some(message) = turn_failure {
        emit_event(
            stdout,
            json_mode,
            &ExecEvent::TurnFailed {
                thread_id,
                turn_id,
                error: ExecError {
                    message: message.clone(),
                },
            },
        )?;
        Err(anyhow!(message))
    } else {
        emit_event(
            stdout,
            json_mode,
            &ExecEvent::TurnCompleted {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                usage: projector.usage_for_turn(&turn_id),
            },
        )?;
        Ok(TurnOutcome {
            final_message: projector.final_message,
        })
    }
}

async fn drain_post_turn_notifications<W: Write>(
    channels: &mut AppServerChannels,
    projector: &mut ExecEventProjector,
    stdout: &mut W,
    json_mode: bool,
    thread_id: &str,
    turn_id: &str,
) -> Result<()> {
    loop {
        match timeout(DRAIN_TIMEOUT, channels.notifications.recv()).await {
            Ok(Some(notification)) => {
                let _ = projector.handle_notification(
                    stdout,
                    json_mode,
                    thread_id,
                    turn_id,
                    notification,
                )?;
            }
            Ok(None) | Err(_) => break,
        }
    }
    Ok(())
}

async fn handle_server_request(
    client: &AppServerClient,
    request: AppServerRequest,
) -> Result<Option<String>> {
    match request.method.as_str() {
        "item/commandExecution/requestApproval" => {
            client
                .respond(
                    request.id,
                    &CommandExecutionRequestApprovalResponse {
                        decision: ApprovalDecision::Decline,
                    },
                )
                .await
                .context("failed to decline command approval request")?;
            Ok(None)
        }
        "item/fileChange/requestApproval" => {
            client
                .respond(
                    request.id,
                    &FileChangeRequestApprovalResponse {
                        decision: ApprovalDecision::Decline,
                    },
                )
                .await
                .context("failed to decline file-change approval request")?;
            Ok(None)
        }
        "item/tool/requestUserInput" => {
            client
                .respond(
                    request.id,
                    &ToolRequestUserInputResponse {
                        answers: HashMap::new(),
                    },
                )
                .await
                .context("failed to answer tool user-input request")?;
            Ok(Some(
                "tool user input requested during non-interactive exec".to_string(),
            ))
        }
        "item/tool/call" => {
            client
                .respond(
                    request.id,
                    &DynamicToolCallResponse {
                        content_items: Vec::new(),
                        success: false,
                    },
                )
                .await
                .context("failed to reject dynamic tool call")?;
            Ok(Some(
                "dynamic tool call requested during non-interactive exec".to_string(),
            ))
        }
        "account/chatgptAuthTokens/refresh" => Ok(Some(
            "auth token refresh requested during non-interactive exec".to_string(),
        )),
        _ => Ok(Some(format!(
            "unsupported app-server request during non-interactive exec: {}",
            request.method
        ))),
    }
}

enum NotificationFlow {
    Continue,
    Completed,
    Failed(String),
}

impl ExecEventProjector {
    fn handle_notification<W: Write>(
        &mut self,
        stdout: &mut W,
        json_mode: bool,
        expected_thread_id: &str,
        expected_turn_id: &str,
        notification: AppServerNotification,
    ) -> Result<NotificationFlow> {
        let method = codex_client::canonical_notification_method(notification.method.as_str());
        let Some(params) = notification.params else {
            return Ok(NotificationFlow::Continue);
        };

        match method {
            "thread/tokenUsage/updated" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                let usage = params
                    .get("tokenUsage")
                    .and_then(|value| value.get("last").or(Some(value)))
                    .map(parse_usage)
                    .unwrap_or_default();
                self.usage_by_turn
                    .insert(expected_turn_id.to_string(), usage);
                Ok(NotificationFlow::Continue)
            }
            "item/started" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                if let Some(item) = params
                    .get("item")
                    .cloned()
                    .and_then(|raw| self.exec_item(raw))
                {
                    emit_event(
                        stdout,
                        json_mode,
                        &ExecEvent::ItemStarted {
                            thread_id: expected_thread_id.to_string(),
                            turn_id: expected_turn_id.to_string(),
                            item,
                        },
                    )?;
                }
                Ok(NotificationFlow::Continue)
            }
            "item/completed" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                if let Some(item) = params
                    .get("item")
                    .cloned()
                    .and_then(|raw| self.exec_item(raw))
                {
                    if item.kind == "agent_message"
                        && let Some(text) = item.text.clone()
                    {
                        self.final_message = Some(text);
                    }
                    self.completed_item_ids.insert(item.id.clone());
                    self.buffered_text.remove(&item.id);
                    self.buffered_command_output.remove(&item.id);
                    emit_event(
                        stdout,
                        json_mode,
                        &ExecEvent::ItemCompleted {
                            thread_id: expected_thread_id.to_string(),
                            turn_id: expected_turn_id.to_string(),
                            item,
                        },
                    )?;
                }
                Ok(NotificationFlow::Continue)
            }
            "item/agentMessage/completed" | "agent_message" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                if let Some((item_id, text)) = extract_agent_message_completion(&params) {
                    self.final_message = Some(text.clone());
                    let item = synthesized_item(&item_id, "agent_message", Some(text), None);
                    self.completed_item_ids.insert(item.id.clone());
                    self.buffered_text.remove(&item.id);
                    emit_event(
                        stdout,
                        json_mode,
                        &ExecEvent::ItemCompleted {
                            thread_id: expected_thread_id.to_string(),
                            turn_id: expected_turn_id.to_string(),
                            item,
                        },
                    )?;
                }
                Ok(NotificationFlow::Continue)
            }
            "agent_message_delta" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                if let Some(item_id) = item_id_from_params(&params) {
                    let delta = params
                        .get("delta")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            params
                                .get("msg")
                                .and_then(|value| value.get("delta"))
                                .and_then(Value::as_str)
                        })
                        .unwrap_or_default();
                    let entry = self.buffered_text.entry(item_id).or_default();
                    entry.kind = "agent_message";
                    entry.text.push_str(delta);
                }
                Ok(NotificationFlow::Continue)
            }
            "agent_reasoning" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                if let Some((item_id, text)) = extract_reasoning_completion(&params) {
                    let item = synthesized_item(&item_id, "reasoning", Some(text), None);
                    self.completed_item_ids.insert(item.id.clone());
                    self.buffered_text.remove(&item.id);
                    emit_event(
                        stdout,
                        json_mode,
                        &ExecEvent::ItemCompleted {
                            thread_id: expected_thread_id.to_string(),
                            turn_id: expected_turn_id.to_string(),
                            item,
                        },
                    )?;
                }
                Ok(NotificationFlow::Continue)
            }
            "agent_reasoning_delta" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                if let Some(item_id) = item_id_from_params(&params) {
                    let delta = params
                        .get("delta")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            params
                                .get("msg")
                                .and_then(|value| value.get("delta"))
                                .and_then(Value::as_str)
                        })
                        .unwrap_or_default();
                    let entry = self.buffered_text.entry(item_id).or_default();
                    entry.kind = "reasoning";
                    entry.text.push_str(delta);
                }
                Ok(NotificationFlow::Continue)
            }
            "item/commandExecution/outputDelta" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                if let Some(item_id) = string_field(&params, "itemId") {
                    let delta = string_field(&params, "delta").unwrap_or_default();
                    self.buffered_command_output
                        .entry(item_id)
                        .or_default()
                        .push_str(&delta);
                }
                Ok(NotificationFlow::Continue)
            }
            "turn/plan/updated" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                let item = ExecItem {
                    id: format!("plan-{expected_turn_id}"),
                    kind: "todo_list".to_string(),
                    text: string_field(&params, "explanation"),
                    command: None,
                    status: None,
                    aggregated_output: None,
                    review: None,
                    query: None,
                    raw: params,
                };
                emit_event(
                    stdout,
                    json_mode,
                    &ExecEvent::ItemUpdated {
                        thread_id: expected_thread_id.to_string(),
                        turn_id: expected_turn_id.to_string(),
                        item,
                    },
                )?;
                Ok(NotificationFlow::Continue)
            }
            "turn/diff/updated" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                let diff = string_field(&params, "diff").unwrap_or_default();
                let item = synthesized_item(
                    &format!("diff-{expected_turn_id}"),
                    "file_change",
                    None,
                    Some(diff),
                );
                emit_event(
                    stdout,
                    json_mode,
                    &ExecEvent::ItemUpdated {
                        thread_id: expected_thread_id.to_string(),
                        turn_id: expected_turn_id.to_string(),
                        item,
                    },
                )?;
                Ok(NotificationFlow::Continue)
            }
            "turn/completed" | "task_complete" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                let status = params
                    .get("turn")
                    .and_then(|value| value.get("status"))
                    .and_then(Value::as_str)
                    .or_else(|| params.get("status").and_then(Value::as_str));
                let error = params
                    .get("turn")
                    .and_then(|value| value.get("error"))
                    .and_then(|value| value.get("message"))
                    .and_then(Value::as_str)
                    .or_else(|| {
                        params
                            .get("error")
                            .and_then(|value| value.get("message"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string);
                if let Some(final_message) = params
                    .get("turn")
                    .and_then(|value| value.get("lastAgentMessage"))
                    .and_then(Value::as_str)
                    .or_else(|| params.get("lastAgentMessage").and_then(Value::as_str))
                    .or_else(|| params.get("message").and_then(Value::as_str))
                    && !final_message.trim().is_empty()
                {
                    self.final_message = Some(final_message.to_string());
                }
                if matches!(status, Some("failed" | "cancelled")) || error.is_some() {
                    Ok(NotificationFlow::Failed(
                        error.unwrap_or_else(|| "turn failed".to_string()),
                    ))
                } else {
                    Ok(NotificationFlow::Completed)
                }
            }
            "turn/error" | "task_failed" => {
                if !matches_turn(&params, expected_thread_id, expected_turn_id) {
                    return Ok(NotificationFlow::Continue);
                }
                let message = params
                    .get("error")
                    .and_then(|value| value.get("message"))
                    .and_then(Value::as_str)
                    .or_else(|| params.get("message").and_then(Value::as_str))
                    .unwrap_or("turn failed")
                    .to_string();
                emit_event(
                    stdout,
                    json_mode,
                    &ExecEvent::Error {
                        thread_id: Some(expected_thread_id.to_string()),
                        turn_id: Some(expected_turn_id.to_string()),
                        message: message.clone(),
                    },
                )?;
                Ok(NotificationFlow::Failed(message))
            }
            _ => Ok(NotificationFlow::Continue),
        }
    }

    fn flush_pending_items<W: Write>(
        &mut self,
        stdout: &mut W,
        json_mode: bool,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<()> {
        let mut pending_ids = self.buffered_text.keys().cloned().collect::<Vec<_>>();
        pending_ids.sort();
        for item_id in pending_ids {
            if self.completed_item_ids.contains(&item_id) {
                continue;
            }
            let Some(pending) = self.buffered_text.remove(&item_id) else {
                continue;
            };
            let item = synthesized_item(&item_id, pending.kind, Some(pending.text), None);
            if item.kind == "agent_message"
                && let Some(text) = item.text.clone()
            {
                self.final_message = Some(text);
            }
            self.completed_item_ids.insert(item.id.clone());
            emit_event(
                stdout,
                json_mode,
                &ExecEvent::ItemCompleted {
                    thread_id: thread_id.to_string(),
                    turn_id: turn_id.to_string(),
                    item,
                },
            )?;
        }

        let mut output_ids = self
            .buffered_command_output
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        output_ids.sort();
        for item_id in output_ids {
            if self.completed_item_ids.contains(&item_id) {
                continue;
            }
            let Some(output) = self.buffered_command_output.remove(&item_id) else {
                continue;
            };
            let item = synthesized_item(&item_id, "command_execution", None, Some(output));
            self.completed_item_ids.insert(item.id.clone());
            emit_event(
                stdout,
                json_mode,
                &ExecEvent::ItemCompleted {
                    thread_id: thread_id.to_string(),
                    turn_id: turn_id.to_string(),
                    item,
                },
            )?;
        }
        Ok(())
    }

    fn usage_for_turn(&self, turn_id: &str) -> Usage {
        self.usage_by_turn.get(turn_id).copied().unwrap_or_default()
    }

    fn exec_item(&self, raw: Value) -> Option<ExecItem> {
        let object = raw.as_object()?;
        let id = object.get("id")?.as_str()?.to_string();
        let raw_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let text = string_field_from_object(object, "text");
        let command = object
            .get("command")
            .and_then(command_field_as_string)
            .or_else(|| string_field_from_object(object, "label"));
        let status = string_field_from_object(object, "status").map(|value| snake_case(&value));
        let aggregated_output = string_field_from_object(object, "aggregatedOutput")
            .or_else(|| self.buffered_command_output.get(&id).cloned());
        let review = string_field_from_object(object, "review");
        let query = string_field_from_object(object, "query");
        let mut normalized_raw = raw;
        if let Some(text) = text.clone() {
            insert_missing_string(&mut normalized_raw, "text", &text);
        }
        if let Some(output) = aggregated_output.clone() {
            insert_missing_string(&mut normalized_raw, "aggregatedOutput", &output);
        }
        Some(ExecItem {
            id,
            kind: snake_case(&raw_type),
            text,
            command,
            status,
            aggregated_output,
            review,
            query,
            raw: normalized_raw,
        })
    }
}

fn emit_event<W: Write>(stdout: &mut W, json_mode: bool, event: &ExecEvent) -> Result<()> {
    if json_mode {
        serde_json::to_writer(&mut *stdout, event).context("failed to encode JSONL event")?;
        stdout
            .write_all(b"\n")
            .context("failed to write JSONL newline")?;
        stdout.flush().context("failed to flush JSONL event")?;
    }
    Ok(())
}

fn command_field_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let command = items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ");
            if command.is_empty() {
                None
            } else {
                Some(command)
            }
        }
        _ => None,
    }
}

fn insert_missing_string(value: &mut Value, key: &str, text: &str) {
    if let Some(object) = value.as_object_mut() {
        object
            .entry(key.to_string())
            .or_insert_with(|| Value::String(text.to_string()));
    }
}

fn synthesized_item(
    item_id: &str,
    kind: &str,
    text: Option<String>,
    aggregated_output: Option<String>,
) -> ExecItem {
    let mut raw = Map::new();
    raw.insert("id".to_string(), Value::String(item_id.to_string()));
    raw.insert("type".to_string(), Value::String(camel_case(kind)));
    if let Some(text_value) = text.clone() {
        raw.insert("text".to_string(), Value::String(text_value));
    }
    if let Some(output) = aggregated_output.clone() {
        raw.insert("aggregatedOutput".to_string(), Value::String(output));
    }
    ExecItem {
        id: item_id.to_string(),
        kind: kind.to_string(),
        text,
        command: None,
        status: None,
        aggregated_output,
        review: None,
        query: None,
        raw: Value::Object(raw),
    }
}

fn matches_turn(params: &Value, expected_thread_id: &str, expected_turn_id: &str) -> bool {
    string_field(params, "threadId").as_deref() == Some(expected_thread_id)
        && (turn_id_from_params(params).as_deref() == Some(expected_turn_id)
            || params
                .get("turn")
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                == Some(expected_turn_id))
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn string_field_from_object(value: &Map<String, Value>, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn turn_id_from_params(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            params
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            params
                .get("msg")
                .and_then(|value| value.get("turn_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn item_id_from_params(params: &Value) -> Option<String> {
    params
        .get("itemId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            params
                .get("item")
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            params
                .get("msg")
                .and_then(|value| value.get("item_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn extract_agent_message_completion(params: &Value) -> Option<(String, String)> {
    let item_id = item_id_from_params(params).unwrap_or_else(|| "agent-message".to_string());
    let text = params
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| params.get("text").and_then(Value::as_str))
        .or_else(|| {
            params
                .get("msg")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            params
                .get("msg")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str)
        })?;
    Some((item_id, text.to_string()))
}

fn extract_reasoning_completion(params: &Value) -> Option<(String, String)> {
    let item_id = item_id_from_params(params).unwrap_or_else(|| "reasoning".to_string());
    let text = params
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| {
            params
                .get("msg")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            params
                .get("msg")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
        })?;
    Some((item_id, text.to_string()))
}

fn parse_usage(value: &Value) -> Usage {
    Usage {
        input_tokens: i64_field(value, "inputTokens").unwrap_or_default(),
        cached_input_tokens: i64_field(value, "cachedInputTokens").unwrap_or_default(),
        output_tokens: i64_field(value, "outputTokens").unwrap_or_default(),
    }
}

fn i64_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}

fn snake_case(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len() + 4);
    for (index, ch) in value.chars().enumerate() {
        if matches!(ch, '/' | '-' | ' ') {
            if !normalized.ends_with('_') {
                normalized.push('_');
            }
            continue;
        }
        if ch.is_uppercase() {
            if index > 0 && !normalized.ends_with('_') {
                normalized.push('_');
            }
            for lower in ch.to_lowercase() {
                normalized.push(lower);
            }
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

fn camel_case(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '_' {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            for upper in ch.to_uppercase() {
                normalized.push(upper);
            }
            uppercase_next = false;
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "Focused unit tests use direct assertions and fixture setup helpers."
)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[test]
    fn snake_case_normalizes_camel_case_types() {
        assert_eq!(snake_case("agentMessage"), "agent_message");
        assert_eq!(snake_case("enteredReviewMode"), "entered_review_mode");
        assert_eq!(snake_case("commandExecution"), "command_execution");
    }

    #[test]
    fn build_request_keeps_ephemeral_and_json_flags() {
        let request = build_run_request(Cli::parse_from([
            "autopilot-codex-exec",
            "--json",
            "--ephemeral",
            "--sandbox",
            "workspace-write",
            "write a test",
        ]))
        .expect("request should build");
        assert!(request.json_mode);
        assert!(request.ephemeral);
        assert_eq!(request.sandbox_mode, Some(SandboxMode::WorkspaceWrite));
    }

    #[test]
    fn build_request_rejects_ephemeral_resume() {
        let error = build_run_request(Cli::parse_from([
            "autopilot-codex-exec",
            "--ephemeral",
            "--thread-id",
            "thread-1",
            "resume",
        ]))
        .expect_err("ephemeral resume should be rejected");
        assert!(format!("{error}").contains("--ephemeral cannot be combined"));
    }

    #[test]
    fn projector_merges_command_output_into_completed_item() {
        let mut projector = ExecEventProjector::default();
        projector
            .buffered_command_output
            .insert("item-1".to_string(), "stdout".to_string());
        let item = projector
            .exec_item(json!({
                "id": "item-1",
                "type": "commandExecution",
                "command": ["git", "status"],
                "status": "completed"
            }))
            .expect("normalized item");
        assert_eq!(item.kind, "command_execution");
        assert_eq!(item.command.as_deref(), Some("git status"));
        assert_eq!(item.aggregated_output.as_deref(), Some("stdout"));
    }

    #[test]
    fn request_parsing_requires_prompt_or_image() {
        let error = build_run_request(Cli::parse_from(["autopilot-codex-exec"]))
            .expect_err("missing prompt should fail");
        assert!(format!("{error}").contains("a prompt or at least one --image is required"));
    }

    #[test]
    fn synthesized_items_round_trip_type_names() {
        let item = synthesized_item("item-1", "agent_message", Some("done".to_string()), None);
        assert_eq!(item.kind, "agent_message");
        assert_eq!(item.raw["type"], Value::String("agentMessage".to_string()));
    }

    #[test]
    fn load_output_schema_reads_json_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("schema.json");
        std::fs::write(&path, "{\"type\":\"object\"}").expect("write schema");
        let schema = load_output_schema(&path).expect("schema should parse");
        assert_eq!(schema["type"], Value::String("object".to_string()));
    }

    #[test]
    fn exec_item_uses_review_and_query_fields_when_present() {
        let projector = ExecEventProjector::default();
        let item = projector
            .exec_item(json!({
                "id": "item-2",
                "type": "webSearch",
                "query": "openagents",
                "review": "looks good"
            }))
            .expect("web search item");
        assert_eq!(item.kind, "web_search");
        assert_eq!(item.query.as_deref(), Some("openagents"));
        assert_eq!(item.review.as_deref(), Some("looks good"));
    }

    #[test]
    fn tool_request_failure_message_is_clear() {
        let message = "tool user input requested during non-interactive exec";
        let event = ExecEvent::Error {
            thread_id: Some("thread-1".to_string()),
            turn_id: Some("turn-1".to_string()),
            message: message.to_string(),
        };
        let encoded = serde_json::to_string(&event).expect("event should encode");
        assert!(encoded.contains(message));
    }

    #[test]
    fn usage_parses_last_scope_payload() {
        let usage = parse_usage(&json!({
            "inputTokens": 12,
            "cachedInputTokens": 3,
            "outputTokens": 7
        }));
        assert_eq!(
            usage,
            Usage {
                input_tokens: 12,
                cached_input_tokens: 3,
                output_tokens: 7,
            }
        );
    }

    #[test]
    fn matches_turn_accepts_nested_turn_ids() {
        let params = json!({
            "threadId": "thread-1",
            "turn": {"id": "turn-1"}
        });
        assert!(matches_turn(&params, "thread-1", "turn-1"));
    }

    #[test]
    fn resolve_prompt_reads_literal_values() {
        let prompt = resolve_prompt(Some("write docs")).expect("prompt should resolve");
        assert_eq!(prompt, "write docs");
    }

    #[test]
    fn extract_agent_message_completion_reads_legacy_shapes() {
        let message = extract_agent_message_completion(&json!({
            "msg": {
                "item_id": "item-3",
                "message": "hello"
            }
        }))
        .expect("legacy completion");
        assert_eq!(message.0, "item-3");
        assert_eq!(message.1, "hello");
    }

    #[test]
    fn camel_case_reconstructs_item_type_keys() {
        assert_eq!(camel_case("agent_message"), "agentMessage");
        assert_eq!(camel_case("command_execution"), "commandExecution");
    }

    #[test]
    fn emit_event_writes_jsonl_lines() {
        let mut buffer = Vec::new();
        emit_event(
            &mut buffer,
            true,
            &ExecEvent::ThreadStarted {
                thread_id: "thread-1".to_string(),
            },
        )
        .expect("event should write");
        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("\"type\":\"thread.started\""));
        assert!(output.ends_with('\n'));
    }

    #[test]
    fn build_turn_input_includes_prompt_and_images() {
        let request = RunRequest {
            prompt: "ship it".to_string(),
            images: vec![PathBuf::from("a.png")],
            model: None,
            cwd: None,
            resume_thread_id: None,
            approval_policy: AskForApproval::Never,
            sandbox_mode: None,
            ephemeral: false,
            output_schema: None,
            last_message_file: None,
            json_mode: true,
            wire_log: None,
        };
        let input = assemble_turn_input(&request);
        assert_eq!(input.len(), 2);
    }

    #[test]
    fn sandbox_policy_conversion_matches_workspace_write() {
        assert!(matches!(
            sandbox_policy_from_mode(Some(SandboxMode::WorkspaceWrite)),
            Some(SandboxPolicy::WorkspaceWrite { .. })
        ));
    }

    #[test]
    fn item_id_from_params_checks_nested_messages() {
        let id = item_id_from_params(&json!({
            "msg": {
                "item_id": "nested"
            }
        }));
        assert_eq!(id.as_deref(), Some("nested"));
    }

    #[test]
    fn run_with_client_streams_jsonl_for_basic_turn() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (client_stream, server_stream) = tokio::io::duplex(32 * 1024);
            let (client_read, client_write) = tokio::io::split(client_stream);
            let (server_read, mut server_write) = tokio::io::split(server_stream);
            let (client, channels) = AppServerClient::connect_with_io(
                Box::new(client_write),
                Box::new(client_read),
                None,
            );

            let saw_turn_start = Arc::new(AtomicBool::new(false));
            let saw_turn_start_clone = Arc::clone(&saw_turn_start);
            let server = tokio::spawn(async move {
                let mut reader = BufReader::new(server_read);
                let mut line = String::new();
                loop {
                    line.clear();
                    let bytes = reader.read_line(&mut line).await.expect("read line");
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = serde_json::from_str(line.trim()).expect("json request");
                    if value.get("id").is_none() {
                        continue;
                    }
                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-1"},
                                "model": "gpt-5.3-codex"
                            }
                        }),
                        "turn/start" => {
                            saw_turn_start_clone.store(true, Ordering::SeqCst);
                            json!({
                                "id": value["id"].clone(),
                                "result": {"turn": {"id": "turn-1"}}
                            })
                        }
                        "turn/interrupt" => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };

                    let encoded = serde_json::to_string(&response).expect("encode response");
                    server_write
                        .write_all(format!("{encoded}\n").as_bytes())
                        .await
                        .expect("write response");
                    server_write.flush().await.expect("flush response");

                    if method == "turn/start" {
                        let notifications = [
                            json!({
                                "jsonrpc": "2.0",
                                "method": "item/agentMessage/delta",
                                "params": {
                                    "threadId": "thread-1",
                                    "turnId": "turn-1",
                                    "itemId": "item-1",
                                    "delta": "hello world"
                                }
                            }),
                            json!({
                                "jsonrpc": "2.0",
                                "method": "thread/tokenUsage/updated",
                                "params": {
                                    "threadId": "thread-1",
                                    "turnId": "turn-1",
                                    "tokenUsage": {
                                        "last": {
                                            "inputTokens": 11,
                                            "cachedInputTokens": 2,
                                            "outputTokens": 5
                                        }
                                    }
                                }
                            }),
                            json!({
                                "jsonrpc": "2.0",
                                "method": "turn/completed",
                                "params": {
                                    "threadId": "thread-1",
                                    "turn": {"id": "turn-1", "status": "completed"}
                                }
                            }),
                        ];
                        for notification in notifications {
                            let encoded =
                                serde_json::to_string(&notification).expect("encode notification");
                            server_write
                                .write_all(format!("{encoded}\n").as_bytes())
                                .await
                                .expect("write notification");
                            server_write.flush().await.expect("flush notification");
                        }
                        break;
                    }
                }
                drop(server_write);
            });

            let request = RunRequest {
                prompt: "say hello".to_string(),
                images: Vec::new(),
                model: Some("gpt-5.3-codex".to_string()),
                cwd: None,
                resume_thread_id: None,
                approval_policy: AskForApproval::Never,
                sandbox_mode: None,
                ephemeral: false,
                output_schema: None,
                last_message_file: None,
                json_mode: true,
                wire_log: None,
            };
            let mut output = Vec::new();
            run_with_client(client, channels, request, &mut output)
                .await
                .expect("runner should succeed");
            assert!(saw_turn_start.load(Ordering::SeqCst));

            let lines = String::from_utf8(output).expect("utf8");
            let events = lines
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).expect("event json"))
                .collect::<Vec<_>>();
            assert_eq!(
                events[0]["type"],
                Value::String("thread.started".to_string())
            );
            assert_eq!(events[1]["type"], Value::String("turn.started".to_string()));
            assert_eq!(
                events[2]["type"],
                Value::String("item.completed".to_string())
            );
            assert_eq!(
                events[2]["item"]["type"],
                Value::String("agent_message".to_string())
            );
            assert_eq!(
                events[2]["item"]["text"],
                Value::String("hello world".to_string())
            );
            assert_eq!(
                events[3]["type"],
                Value::String("turn.completed".to_string())
            );
            assert_eq!(events[3]["usage"]["input_tokens"], Value::Number(11.into()));

            server.abort();
        });
    }
}
