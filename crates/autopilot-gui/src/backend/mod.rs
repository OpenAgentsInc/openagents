use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use autopilot::daemon::config::{DaemonConfig, WorkerCommand};
use autopilot::daemon::supervisor::{DaemonMetrics, SharedMetrics, WorkerSupervisor};
use autopilot::metrics::{MetricsDb, SessionMetrics, SummaryStats};
use autopilot::parallel::{AgentInfo, ParallelConfig, Platform};
use claude_agent_sdk::{
    SdkAuthStatusMessage, SdkResultMessage, SdkStreamEvent, SdkSystemMessage,
    SdkToolProgressMessage,
};
use issues::issue::{list_issues, Status as IssueStatus};
use serde::Deserialize;
use serde_json::Value;
use wgpui::components::atoms::{ToolStatus, ToolType};

use crate::state::{ChatEntry, IssueSummary, ParallelPlatformInfo, ToolCallData};

#[derive(Clone, Debug)]
pub enum BackendEvent {
    Metrics {
        sessions: Vec<SessionMetrics>,
        summary: SummaryStats,
    },
    Chat {
        path: Option<PathBuf>,
        session_id: Option<String>,
        entries: Vec<ChatEntry>,
    },
    Agents {
        agents: Vec<AgentInfo>,
    },
    Issues {
        issues: Vec<IssueSummary>,
    },
    Platform {
        info: ParallelPlatformInfo,
    },
    FullAuto {
        metrics: Option<DaemonMetrics>,
    },
    PromptStatus {
        running: bool,
        last_prompt: Option<String>,
    },
    Status {
        message: String,
    },
}

#[derive(Clone, Debug)]
pub enum BackendCommand {
    StartFullAuto,
    StopFullAuto,
    StartParallel { count: usize },
    StopParallel,
    RunPrompt { prompt: String },
    AbortPrompt,
}

#[derive(Clone, Debug)]
pub struct BackendConfig {
    pub metrics_path: PathBuf,
    pub issues_path: PathBuf,
    pub logs_dir: PathBuf,
    pub refresh_interval: Duration,
    pub agents_interval: Duration,
    pub issues_interval: Duration,
    pub max_chat_entries: usize,
    pub workdir: PathBuf,
    pub project: Option<String>,
    pub model: String,
    pub worker_command: WorkerCommand,
}

impl Default for BackendConfig {
    fn default() -> Self {
        let workdir = autopilot::find_workspace_root();
        let metrics_path = autopilot::metrics::default_db_path();
        let issues_path = autopilot::default_db_path();
        let logs_dir = workdir.join("docs").join("logs");

        Self {
            metrics_path,
            issues_path,
            logs_dir,
            refresh_interval: Duration::from_secs(2),
            agents_interval: Duration::from_secs(5),
            issues_interval: Duration::from_secs(5),
            max_chat_entries: 240,
            workdir,
            project: None,
            model: "sonnet".to_string(),
            worker_command: WorkerCommand::default(),
        }
    }
}

pub struct BackendHandle {
    pub receiver: Receiver<BackendEvent>,
    pub sender: Sender<BackendCommand>,
}

pub fn start_backend(config: BackendConfig) -> BackendHandle {
    let (event_tx, event_rx) = mpsc::channel();
    let (cmd_tx, cmd_rx) = mpsc::channel();

    thread::spawn(move || run_backend_loop(config, event_tx, cmd_rx));

    BackendHandle {
        receiver: event_rx,
        sender: cmd_tx,
    }
}

struct FullAutoManager {
    metrics: SharedMetrics,
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
    _task: tokio::task::JoinHandle<()>,
}

impl FullAutoManager {
    fn start(runtime: &tokio::runtime::Runtime, config: DaemonConfig) -> Self {
        let metrics: SharedMetrics =
            Arc::new(std::sync::RwLock::new(DaemonMetrics::default()));
        let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel(1);
        let metrics_clone = metrics.clone();

        let task = runtime.spawn(async move {
            let mut supervisor = WorkerSupervisor::new(config);
            if let Err(err) = supervisor.run(shutdown_rx, Some(metrics_clone)).await {
                eprintln!("Full auto supervisor error: {}", err);
            }
        });

        Self {
            metrics,
            shutdown_tx,
            _task: task,
        }
    }

    fn stop(&mut self, runtime: &tokio::runtime::Runtime) {
        let _ = runtime.block_on(self.shutdown_tx.send(()));
    }

    fn metrics(&self) -> Option<DaemonMetrics> {
        self.metrics.read().ok().map(|guard| guard.clone())
    }
}

struct PromptRunner {
    child: Option<Child>,
    last_prompt: Option<String>,
}

impl PromptRunner {
    fn new() -> Self {
        Self {
            child: None,
            last_prompt: None,
        }
    }

    fn is_running(&self) -> bool {
        self.child.is_some()
    }

    fn start(
        &mut self,
        command: &WorkerCommand,
        config: &BackendConfig,
        prompt: String,
    ) -> anyhow::Result<()> {
        if self.child.is_some() {
            anyhow::bail!("Prompt already running");
        }

        let mut cmd = build_prompt_command(command, config, &prompt);
        cmd.current_dir(&config.workdir);

        let child = cmd.spawn()?;
        self.child = Some(child);
        self.last_prompt = Some(prompt);

        Ok(())
    }

    fn abort(&mut self) -> anyhow::Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    fn poll(&mut self) -> Option<std::process::ExitStatus> {
        let Some(child) = self.child.as_mut() else {
            return None;
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                self.child = None;
                Some(status)
            }
            Ok(None) => None,
            Err(_) => {
                self.child = None;
                None
            }
        }
    }
}

fn run_backend_loop(
    config: BackendConfig,
    tx: mpsc::Sender<BackendEvent>,
    cmd_rx: mpsc::Receiver<BackendCommand>,
) {
    let mut metrics_db = None;
    let mut last_agents = Instant::now() - config.agents_interval;
    let mut last_issues = Instant::now() - config.issues_interval;

    let runtime = tokio::runtime::Runtime::new().ok();
    let mut full_auto: Option<FullAutoManager> = None;
    let mut prompt_runner = PromptRunner::new();
    let mut last_prompt_running = None;

    let platform_info = build_platform_info();
    let _ = tx.send(BackendEvent::Platform { info: platform_info });

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            handle_command(
                cmd,
                &config,
                &tx,
                runtime.as_ref(),
                &mut full_auto,
                &mut prompt_runner,
            );
        }

        if let Some(status) = prompt_runner.poll() {
            let _ = tx.send(BackendEvent::Status {
                message: format!("Prompt finished: {}", status),
            });
        }

        let prompt_running = prompt_runner.is_running();
        if last_prompt_running != Some(prompt_running) {
            last_prompt_running = Some(prompt_running);
            let _ = tx.send(BackendEvent::PromptStatus {
                running: prompt_running,
                last_prompt: prompt_runner.last_prompt.clone(),
            });
        }

        if metrics_db.is_none() {
            match MetricsDb::open(&config.metrics_path) {
                Ok(db) => metrics_db = Some(db),
                Err(err) => {
                    let _ = tx.send(BackendEvent::Status {
                        message: format!("Metrics database unavailable: {}", err),
                    });
                }
            }
        }

        if let Some(db) = metrics_db.as_ref() {
            let sessions = db.get_recent_sessions(200).unwrap_or_default();
            let summary = db.get_summary_stats().unwrap_or_default();
            let _ = tx.send(BackendEvent::Metrics { sessions, summary });
        }

        let (log_path, log_session, chat_entries) =
            load_latest_chat(&config.logs_dir, config.max_chat_entries);
        let _ = tx.send(BackendEvent::Chat {
            path: log_path,
            session_id: log_session,
            entries: chat_entries,
        });

        if last_agents.elapsed() >= config.agents_interval {
            if let Some(runtime) = runtime.as_ref() {
                match runtime.block_on(autopilot::parallel::list_agents()) {
                    Ok(agents) => {
                        let _ = tx.send(BackendEvent::Agents { agents });
                    }
                    Err(err) => {
                        let _ = tx.send(BackendEvent::Status {
                            message: format!("Parallel agents unavailable: {}", err),
                        });
                    }
                }
            }
            last_agents = Instant::now();
        }

        if last_issues.elapsed() >= config.issues_interval {
            match std::fs::metadata(&config.issues_path) {
                Ok(_) => {
                    if let Ok(conn) = rusqlite::Connection::open(&config.issues_path) {
                        let issues = list_issues(&conn, Some(IssueStatus::Open))
                            .unwrap_or_default()
                            .into_iter()
                            .map(|issue| IssueSummary {
                                number: issue.number,
                                title: issue.title,
                                priority: issue.priority.as_str().to_string(),
                            })
                            .collect();
                        let _ = tx.send(BackendEvent::Issues { issues });
                    }
                }
                Err(err) => {
                    let _ = tx.send(BackendEvent::Status {
                        message: format!("Issues database unavailable: {}", err),
                    });
                }
            }
            last_issues = Instant::now();
        }

        let full_auto_metrics = full_auto.as_ref().and_then(|mgr| mgr.metrics());
        let _ = tx.send(BackendEvent::FullAuto {
            metrics: full_auto_metrics,
        });

        thread::sleep(config.refresh_interval);
    }
}

fn handle_command(
    cmd: BackendCommand,
    config: &BackendConfig,
    tx: &Sender<BackendEvent>,
    runtime: Option<&tokio::runtime::Runtime>,
    full_auto: &mut Option<FullAutoManager>,
    prompt_runner: &mut PromptRunner,
) {
    match cmd {
        BackendCommand::StartFullAuto => {
            if full_auto.is_some() {
                return;
            }
            let Some(runtime) = runtime else {
                let _ = tx.send(BackendEvent::Status {
                    message: "Tokio runtime unavailable for full auto".to_string(),
                });
                return;
            };
            let mut daemon_config = DaemonConfig::default();
            daemon_config.worker_command = config.worker_command.clone();
            daemon_config.working_dir = config.workdir.clone();
            daemon_config.project = config.project.clone();
            daemon_config.model = config.model.clone();
            let manager = FullAutoManager::start(runtime, daemon_config);
            *full_auto = Some(manager);
            let _ = tx.send(BackendEvent::Status {
                message: "Full auto started".to_string(),
            });
        }
        BackendCommand::StopFullAuto => {
            if let Some(manager) = full_auto.as_mut() {
                if let Some(runtime) = runtime {
                    manager.stop(runtime);
                }
                *full_auto = None;
                let _ = tx.send(BackendEvent::Status {
                    message: "Full auto stopped".to_string(),
                });
            }
        }
        BackendCommand::StartParallel { count } => {
            if let Some(runtime) = runtime {
                match runtime.block_on(autopilot::parallel::start_agents(count)) {
                    Ok(_) => {
                        let _ = tx.send(BackendEvent::Status {
                            message: format!("Parallel agents started: {}", count),
                        });
                    }
                    Err(err) => {
                        let _ = tx.send(BackendEvent::Status {
                            message: format!("Parallel start failed: {}", err),
                        });
                    }
                }
            }
        }
        BackendCommand::StopParallel => {
            if let Some(runtime) = runtime {
                match runtime.block_on(autopilot::parallel::stop_agents()) {
                    Ok(_) => {
                        let _ = tx.send(BackendEvent::Status {
                            message: "Parallel agents stopped".to_string(),
                        });
                    }
                    Err(err) => {
                        let _ = tx.send(BackendEvent::Status {
                            message: format!("Parallel stop failed: {}", err),
                        });
                    }
                }
            }
        }
        BackendCommand::RunPrompt { prompt } => {
            let result = prompt_runner.start(&config.worker_command, config, prompt);
            if let Err(err) = result {
                let _ = tx.send(BackendEvent::Status {
                    message: format!("Prompt failed: {}", err),
                });
            } else {
                let _ = tx.send(BackendEvent::Status {
                    message: "Prompt started".to_string(),
                });
            }
        }
        BackendCommand::AbortPrompt => {
            if let Err(err) = prompt_runner.abort() {
                let _ = tx.send(BackendEvent::Status {
                    message: format!("Abort failed: {}", err),
                });
            } else {
                let _ = tx.send(BackendEvent::Status {
                    message: "Prompt aborted".to_string(),
                });
            }
        }
    }
}

fn build_platform_info() -> ParallelPlatformInfo {
    let platform = Platform::detect();
    let config = ParallelConfig::for_platform(platform, platform.max_agents());
    ParallelPlatformInfo {
        platform: format!("{:?}", platform),
        max_agents: platform.max_agents(),
        memory_limit: config.memory_limit,
        cpu_limit: config.cpu_limit,
    }
}

fn build_prompt_command(
    worker_command: &WorkerCommand,
    config: &BackendConfig,
    prompt: &str,
) -> Command {
    match worker_command {
        WorkerCommand::Cargo { manifest_path } => {
            let mut cmd = Command::new("cargo");
            cmd.arg("run");
            cmd.arg("--bin").arg("openagents");
            cmd.arg("--");
            cmd.arg("autopilot");
            cmd.arg("run");
            cmd.arg("--model").arg(&config.model);
            if let Some(project) = &config.project {
                cmd.arg("--project").arg(project);
            }
            if let Some(manifest) = manifest_path {
                cmd.arg("--manifest-path").arg(manifest);
            }
            cmd.arg(prompt);
            cmd
        }
        WorkerCommand::Binary { path } => {
            let mut cmd = Command::new(path);
            if is_openagents_binary(path) {
                cmd.arg("autopilot");
                cmd.arg("run");
            } else {
                cmd.arg("run");
            }
            cmd.arg("--model").arg(&config.model);
            if let Some(project) = &config.project {
                cmd.arg("--project").arg(project);
            }
            cmd.arg(prompt);
            cmd
        }
    }
}

fn is_openagents_binary(path: &Path) -> bool {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case("openagents"))
        .unwrap_or(false)
}

#[derive(Deserialize)]
struct JsonlRecord {
    #[serde(rename = "type")]
    kind: String,
    message: Value,
    timestamp: Option<String>,
}

struct ChatAssembler {
    entries: Vec<ChatEntry>,
    pending_tools: HashMap<String, usize>,
    streaming_text: Option<usize>,
    streaming_tool_id: Option<String>,
    session_id: Option<String>,
}

impl ChatAssembler {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            pending_tools: HashMap::new(),
            streaming_text: None,
            streaming_tool_id: None,
            session_id: None,
        }
    }

    fn note_session_id(&mut self, session_id: &str) {
        if self.session_id.is_none() {
            self.session_id = Some(session_id.to_string());
        }
    }

    fn push_tool_call(&mut self, tool: ToolCallData) {
        let idx = self.entries.len();
        self.pending_tools.insert(tool.id.clone(), idx);
        self.entries.push(ChatEntry::ToolCall(tool));
    }

    fn update_tool_call(
        &mut self,
        tool_use_id: &str,
        status: ToolStatus,
        input: Option<String>,
        output: Option<String>,
    ) {
        if let Some(idx) = self.pending_tools.get(tool_use_id).copied() {
            if let Some(ChatEntry::ToolCall(tool)) = self.entries.get_mut(idx) {
                tool.status = status;
                if tool.input.is_none() && input.is_some() {
                    tool.input = input;
                }
                if output.is_some() {
                    tool.output = output;
                }
            }
            if matches!(status, ToolStatus::Success | ToolStatus::Error) {
                self.pending_tools.remove(tool_use_id);
            }
        } else if output.is_some() || input.is_some() {
            let tool = ToolCallData {
                id: tool_use_id.to_string(),
                name: "(unknown)".to_string(),
                tool_type: ToolType::Unknown,
                status,
                input,
                output,
            };
            self.entries.push(ChatEntry::ToolCall(tool));
        }
    }
}

fn load_latest_chat(
    logs_dir: &Path,
    max_entries: usize,
) -> (Option<PathBuf>, Option<String>, Vec<ChatEntry>) {
    let path = find_latest_log(logs_dir, &["jsonl", "rlog"]);
    let Some(path) = path else {
        return (None, None, Vec::new());
    };

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => return (Some(path), None, Vec::new()),
    };

    let content = String::from_utf8_lossy(&bytes);

    if path.extension().map(|ext| ext == "jsonl").unwrap_or(false) {
        let (entries, session_id) = parse_jsonl_chat(&content, max_entries);
        return (Some(path), session_id, entries);
    }

    let session_id = autopilot::extract_session_id_from_rlog(&path)
        .ok()
        .and_then(|id| id);
    let mut lines: Vec<ChatEntry> = content
        .lines()
        .map(|line| ChatEntry::System {
            text: line.to_string(),
            timestamp: None,
        })
        .collect();

    if lines.len() > max_entries {
        lines = lines.split_off(lines.len() - max_entries);
    }

    (Some(path), session_id, lines)
}

fn parse_jsonl_chat(content: &str, max_entries: usize) -> (Vec<ChatEntry>, Option<String>) {
    let mut assembler = ChatAssembler::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let record: JsonlRecord = match serde_json::from_str(line) {
            Ok(record) => record,
            Err(_) => continue,
        };

        match record.kind.as_str() {
            "assistant" => {
                parse_assistant_message(&mut assembler, record.message, record.timestamp);
            }
            "user" => {
                parse_user_message(&mut assembler, record.message, record.timestamp);
            }
            "system" => {
                if let Ok(sys) = serde_json::from_value::<SdkSystemMessage>(record.message) {
                    parse_system_message(&mut assembler, sys, record.timestamp);
                }
            }
            "result" => {
                if let Ok(result) = serde_json::from_value::<SdkResultMessage>(record.message) {
                    parse_result_message(&mut assembler, result, record.timestamp);
                }
            }
            "tool_progress" => {
                if let Ok(progress) =
                    serde_json::from_value::<SdkToolProgressMessage>(record.message)
                {
                    parse_tool_progress(&mut assembler, progress);
                }
            }
            "stream_event" => {
                if let Ok(event) = serde_json::from_value::<SdkStreamEvent>(record.message) {
                    parse_stream_event(&mut assembler, event, record.timestamp);
                }
            }
            "auth_status" => {
                if let Ok(auth) = serde_json::from_value::<SdkAuthStatusMessage>(record.message) {
                    parse_auth_status(&mut assembler, auth, record.timestamp);
                }
            }
            _ => {}
        }
    }

    if assembler.entries.len() > max_entries {
        let start = assembler.entries.len() - max_entries;
        assembler.entries = assembler.entries.split_off(start);
    }

    (assembler.entries, assembler.session_id)
}

fn parse_assistant_message(
    assembler: &mut ChatAssembler,
    message: Value,
    timestamp: Option<String>,
) {
    let content = message.get("content");
    let mut text_blocks = Vec::new();

    if let Some(Value::Array(blocks)) = content {
        for block in blocks {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        text_blocks.push(text.to_string());
                    }
                }
                "tool_use" => {
                    let tool_id = block
                        .get("id")
                        .or_else(|| block.get("tool_use_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool_use");
                    let tool_name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");
                    let input = block.get("input").cloned().unwrap_or(Value::Null);

                    if assembler.pending_tools.contains_key(tool_id) {
                        assembler.update_tool_call(
                            tool_id,
                            ToolStatus::Running,
                            Some(format_tool_input(tool_name, &input)),
                            None,
                        );
                    } else {
                        let tool = ToolCallData {
                            id: tool_id.to_string(),
                            name: tool_name.to_string(),
                            tool_type: tool_type_from_name(tool_name),
                            status: ToolStatus::Pending,
                            input: Some(format_tool_input(tool_name, &input)),
                            output: None,
                        };
                        assembler.push_tool_call(tool);
                    }
                }
                _ => {}
            }
        }
    } else if let Some(Value::String(text)) = content {
        text_blocks.push(text.clone());
    }

    if !text_blocks.is_empty() {
        let text = text_blocks.join("\n");
        if let Some(idx) = assembler.streaming_text.take() {
            if let Some(ChatEntry::Assistant {
                text: existing,
                streaming,
                timestamp: ts,
            }) = assembler.entries.get_mut(idx)
            {
                *existing = text;
                *streaming = false;
                if ts.is_none() {
                    *ts = timestamp.clone();
                }
                return;
            }
        }
        assembler.entries.push(ChatEntry::Assistant {
            text,
            timestamp,
            streaming: false,
        });
    }
}

fn parse_user_message(
    assembler: &mut ChatAssembler,
    message: Value,
    timestamp: Option<String>,
) {
    let content = message.get("content");

    if let Some(Value::Array(blocks)) = content {
        for block in blocks {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if block_type == "tool_result" {
                let tool_id = block
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool_use");
                let is_error = block
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let output = extract_tool_result(block);
                let status = if is_error {
                    ToolStatus::Error
                } else {
                    ToolStatus::Success
                };
                assembler.update_tool_call(tool_id, status, None, Some(output));
                continue;
            }
            if block_type == "text" {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    assembler.entries.push(ChatEntry::User {
                        text: text.to_string(),
                        timestamp: timestamp.clone(),
                    });
                }
            }
        }
    } else if let Some(Value::String(text)) = content {
        assembler.entries.push(ChatEntry::User {
            text: text.clone(),
            timestamp,
        });
    }
}

fn parse_system_message(
    assembler: &mut ChatAssembler,
    sys: SdkSystemMessage,
    timestamp: Option<String>,
) {
    use claude_agent_sdk::SdkSystemMessage::*;

    let session_id = match &sys {
        Init(init) => Some(init.session_id.as_str()),
        Status(status) => Some(status.session_id.as_str()),
        HookResponse(hook) => Some(hook.session_id.as_str()),
        ApiError(err) => Some(err.session_id.as_str()),
        StopHookSummary(summary) => Some(summary.session_id.as_str()),
        Informational(info) => Some(info.session_id.as_str()),
        LocalCommand(cmd) => Some(cmd.session_id.as_str()),
        CompactBoundary(boundary) => Some(boundary.session_id.as_str()),
    };
    if let Some(session_id) = session_id {
        assembler.note_session_id(session_id);
    }

    let text = match sys {
        Init(init) => format!("Session init: {}", init.session_id),
        Status(status) => format!("Status: {:?}", status.status),
        HookResponse(hook) => format!("Hook: {} ({})", hook.hook_name, hook.hook_event),
        ApiError(err) => err
            .message
            .unwrap_or_else(|| "API error".to_string()),
        StopHookSummary(summary) => summary
            .summary
            .unwrap_or_else(|| "Session stop".to_string()),
        Informational(info) => info.message,
        LocalCommand(cmd) => cmd
            .command
            .map(|c| format!("Local command: {}", c))
            .unwrap_or_else(|| "Local command".to_string()),
        CompactBoundary(boundary) => {
            format!("Compact boundary: {}", boundary.compact_metadata.trigger)
        }
    };

    assembler.entries.push(ChatEntry::System { text, timestamp });
}

fn parse_result_message(
    assembler: &mut ChatAssembler,
    result: SdkResultMessage,
    timestamp: Option<String>,
) {
    use claude_agent_sdk::SdkResultMessage::*;

    let session_id = match &result {
        Success(success) => Some(success.session_id.as_str()),
        ErrorDuringExecution(err)
        | ErrorMaxTurns(err)
        | ErrorMaxBudget(err)
        | ErrorMaxStructuredOutputRetries(err) => Some(err.session_id.as_str()),
    };
    if let Some(session_id) = session_id {
        assembler.note_session_id(session_id);
    }

    let text = match result {
        Success(success) => format!("Session complete: {}", success.result),
        ErrorDuringExecution(err)
        | ErrorMaxTurns(err)
        | ErrorMaxBudget(err)
        | ErrorMaxStructuredOutputRetries(err) => {
            let summary = if err.errors.is_empty() {
                "Session error".to_string()
            } else {
                err.errors.join("; ")
            };
            format!("Session error: {}", summary)
        }
    };

    assembler.entries.push(ChatEntry::System { text, timestamp });
}

fn parse_tool_progress(assembler: &mut ChatAssembler, progress: SdkToolProgressMessage) {
    assembler.note_session_id(&progress.session_id);
    assembler.update_tool_call(
        &progress.tool_use_id,
        ToolStatus::Running,
        None,
        None,
    );
}

fn parse_stream_event(
    assembler: &mut ChatAssembler,
    event: SdkStreamEvent,
    timestamp: Option<String>,
) {
    assembler.note_session_id(&event.session_id);
    let event_type = event.event.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match event_type {
        "content_block_start" => {
            if let Some(block) = event.event.get("content_block") {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if block_type == "text" {
                    let idx = assembler.entries.len();
                    assembler.entries.push(ChatEntry::Assistant {
                        text: String::new(),
                        timestamp,
                        streaming: true,
                    });
                    assembler.streaming_text = Some(idx);
                } else if block_type == "tool_use" {
                    let tool_id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool_use");
                    let tool_name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");
                    let tool = ToolCallData {
                        id: tool_id.to_string(),
                        name: tool_name.to_string(),
                        tool_type: tool_type_from_name(tool_name),
                        status: ToolStatus::Running,
                        input: Some(String::new()),
                        output: None,
                    };
                    assembler.streaming_tool_id = Some(tool_id.to_string());
                    assembler.push_tool_call(tool);
                }
            }
        }
        "content_block_delta" => {
            if let Some(delta) = event.event.get("delta") {
                let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if delta_type == "text_delta" {
                    let text = delta.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(idx) = assembler.streaming_text {
                        if let Some(ChatEntry::Assistant { text: current, .. }) =
                            assembler.entries.get_mut(idx)
                        {
                            current.push_str(text);
                        }
                    } else if !text.is_empty() {
                        let idx = assembler.entries.len();
                        assembler.entries.push(ChatEntry::Assistant {
                            text: text.to_string(),
                            timestamp,
                            streaming: true,
                        });
                        assembler.streaming_text = Some(idx);
                    }
                } else if delta_type == "input_json_delta" {
                    if let Some(tool_id) = assembler.streaming_tool_id.clone() {
                        if let Some(idx) = assembler.pending_tools.get(&tool_id).copied() {
                            if let Some(ChatEntry::ToolCall(tool)) =
                                assembler.entries.get_mut(idx)
                            {
                                let chunk = delta
                                    .get("partial_json")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                tool.input
                                    .get_or_insert_with(String::new)
                                    .push_str(chunk);
                            }
                        }
                    }
                }
            }
        }
        "content_block_stop" => {
            if let Some(idx) = assembler.streaming_text {
                if let Some(ChatEntry::Assistant { streaming, .. }) =
                    assembler.entries.get_mut(idx)
                {
                    *streaming = false;
                }
            }
            assembler.streaming_tool_id = None;
        }
        _ => {}
    }
}

fn parse_auth_status(
    assembler: &mut ChatAssembler,
    auth: SdkAuthStatusMessage,
    timestamp: Option<String>,
) {
    assembler.note_session_id(&auth.session_id);
    let text = if auth.is_authenticating {
        "Authenticating with provider...".to_string()
    } else if let Some(err) = auth.error {
        format!("Auth error: {}", err)
    } else {
        "Authentication complete.".to_string()
    };
    assembler.entries.push(ChatEntry::System { text, timestamp });
}

fn tool_type_from_name(name: &str) -> ToolType {
    match name.to_lowercase().as_str() {
        "read" => ToolType::Read,
        "write" => ToolType::Write,
        "edit" => ToolType::Edit,
        "bash" => ToolType::Bash,
        "glob" => ToolType::Glob,
        "grep" => ToolType::Grep,
        "list" | "ls" => ToolType::List,
        "search" => ToolType::Search,
        "task" => ToolType::Task,
        "web_fetch" | "webfetch" => ToolType::WebFetch,
        _ => ToolType::Unknown,
    }
}

fn format_tool_input(tool_name: &str, input: &Value) -> String {
    match tool_name {
        "Bash" => input
            .get("command")
            .and_then(|c| c.as_str())
            .map(|c| truncate_text(&format!("cmd=\"{}\"", c), 160))
            .unwrap_or_default(),
        "Read" | "Write" | "Edit" => input
            .get("file_path")
            .and_then(|p| p.as_str())
            .map(|p| truncate_text(&format!("file_path={}", p), 160))
            .unwrap_or_default(),
        "Glob" => input
            .get("pattern")
            .and_then(|p| p.as_str())
            .map(|p| truncate_text(&format!("pattern=\"{}\"", p), 160))
            .unwrap_or_default(),
        "Grep" => input
            .get("pattern")
            .and_then(|p| p.as_str())
            .map(|p| truncate_text(&format!("pattern=\"{}\"", p), 160))
            .unwrap_or_default(),
        "Task" => input
            .get("description")
            .and_then(|d| d.as_str())
            .map(|d| truncate_text(&format!("desc=\"{}\"", d), 160))
            .unwrap_or_default(),
        _ => truncate_text(&value_to_string(input), 160),
    }
}

fn extract_tool_result(block: &Value) -> String {
    if let Some(content) = block.get("content") {
        return truncate_text(&value_to_string(content), 240);
    }
    String::new()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_default(),
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let mut out = text.chars().take(max_len.saturating_sub(3)).collect::<String>();
    out.push_str("...");
    out
}

fn find_latest_log(logs_dir: &Path, extensions: &[&str]) -> Option<PathBuf> {
    let mut dirs: Vec<_> = std::fs::read_dir(logs_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect();

    dirs.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    let latest_dir = dirs.first()?.path();

    let mut logs: Vec<_> = std::fs::read_dir(&latest_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| extensions.contains(&ext))
                .unwrap_or(false)
        })
        .collect();

    logs.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    logs.first().map(|entry| entry.path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn record(kind: &str, message: serde_json::Value) -> String {
        json!({
            "type": kind,
            "message": message,
            "timestamp": "2025-01-01T00:00:00Z"
        })
        .to_string()
    }

    #[test]
    fn test_parse_jsonl_chat_stream_and_tool_calls() {
        let system_init = json!({
            "subtype": "init",
            "agents": ["default"],
            "apiKeySource": "none",
            "betas": null,
            "claude_code_version": "1.0.0",
            "cwd": "/workspace",
            "tools": ["Bash"],
            "mcp_servers": [],
            "model": "claude-sonnet-4-5-20250929",
            "permissionMode": "bypassPermissions",
            "slash_commands": [],
            "output_style": "default",
            "skills": [],
            "plugins": [],
            "uuid": "00000000-0000-0000-0000-000000000000",
            "session_id": "session-123"
        });

        let user_message = json!({
            "role": "user",
            "content": [
                { "type": "text", "text": "run command" }
            ]
        });

        let stream_start = json!({
            "type": "content_block_start",
            "content_block": { "type": "text" }
        });
        let stream_delta = json!({
            "type": "content_block_delta",
            "delta": { "type": "text_delta", "text": "Working..." }
        });
        let stream_stop = json!({
            "type": "content_block_stop"
        });

        let assistant_message = json!({
            "role": "assistant",
            "content": [
                { "type": "text", "text": "Working..." },
                { "type": "tool_use", "id": "toolu_1", "name": "Bash", "input": { "command": "ls -la" } }
            ]
        });

        let tool_result = json!({
            "role": "user",
            "content": [
                { "type": "tool_result", "tool_use_id": "toolu_1", "is_error": false, "content": "ok" }
            ]
        });

        let content = vec![
            record("system", system_init),
            record("user", user_message),
            record("stream_event", stream_start),
            record("stream_event", stream_delta),
            record("stream_event", stream_stop),
            record("assistant", assistant_message),
            record("user", tool_result),
        ]
        .join("\n");

        let (entries, session_id) = parse_jsonl_chat(&content, 50);

        assert_eq!(session_id.as_deref(), Some("session-123"));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| matches!(entry, ChatEntry::Assistant { .. }))
                .count(),
            1
        );

        let assistant = entries.iter().find_map(|entry| match entry {
            ChatEntry::Assistant {
                text,
                streaming,
                ..
            } => Some((text, streaming)),
            _ => None,
        });
        let (assistant_text, streaming) = assistant.expect("assistant entry");
        assert!(assistant_text.contains("Working..."));
        assert!(!*streaming);

        let tool = entries.iter().find_map(|entry| match entry {
            ChatEntry::ToolCall(tool) => Some(tool),
            _ => None,
        });
        let tool = tool.expect("tool call entry");
        assert_eq!(tool.name, "Bash");
        assert_eq!(tool.status, ToolStatus::Success);
        assert!(tool.input.as_deref().unwrap_or_default().contains("cmd="));
        assert!(tool.output.as_deref().unwrap_or_default().contains("ok"));
    }

    #[test]
    fn test_build_prompt_command_cargo_uses_openagents() {
        let mut config = BackendConfig::default();
        config.model = "sonnet".to_string();
        let command = build_prompt_command(
            &WorkerCommand::Cargo {
                manifest_path: Some(PathBuf::from("Cargo.toml")),
            },
            &config,
            "do the thing",
        );

        let program = command.get_program().to_string_lossy().to_string();
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert_eq!(program, "cargo");
        assert!(
            args.iter()
                .take(6)
                .map(String::as_str)
                .eq(["run", "--bin", "openagents", "--", "autopilot", "run"])
        );
        assert!(args.contains(&"do the thing".to_string()));
    }

    #[test]
    fn test_build_prompt_command_binary_openagents() {
        let config = BackendConfig::default();
        let command = build_prompt_command(
            &WorkerCommand::Binary {
                path: PathBuf::from("/usr/bin/openagents"),
            },
            &config,
            "hello",
        );

        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(
            args.iter()
                .take(2)
                .map(String::as_str)
                .eq(["autopilot", "run"])
        );
    }

    #[test]
    fn test_build_prompt_command_binary_autopilot() {
        let config = BackendConfig::default();
        let command = build_prompt_command(
            &WorkerCommand::Binary {
                path: PathBuf::from("/usr/bin/autopilot"),
            },
            &config,
            "hello",
        );

        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(
            args.iter()
                .take(1)
                .map(String::as_str)
                .eq(["run"])
        );
    }

    #[test]
    fn test_find_latest_log_prefers_latest_dir_and_time() {
        let temp = tempfile::tempdir().expect("temp dir");
        let logs_dir = temp.path();
        let older = logs_dir.join("20250101");
        let newer = logs_dir.join("20250102");
        std::fs::create_dir_all(&older).expect("older");
        std::fs::create_dir_all(&newer).expect("newer");

        let older_file = older.join("000000-old.jsonl");
        std::fs::write(&older_file, "older").expect("older file");

        let first = newer.join("000001-first.jsonl");
        std::fs::write(&first, "first").expect("first file");
        std::thread::sleep(Duration::from_millis(10));
        let second = newer.join("000002-second.jsonl");
        std::fs::write(&second, "second").expect("second file");

        let found = find_latest_log(logs_dir, &["jsonl"]).expect("latest log");
        assert_eq!(found, second);
    }
}
