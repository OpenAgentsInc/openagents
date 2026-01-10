fn wait_for_stream(deadline: Option<Instant>) -> FsResult<bool> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = deadline;
        return Ok(false);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                return Ok(false);
            }
        }
        std::thread::sleep(Duration::from_millis(25));
        Ok(true)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClaudeError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("no provider available for model {model}: {reason}")]
    NoProviderAvailable { model: String, reason: String },
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("session not found")]
    SessionNotFound,
    #[error("budget exceeded")]
    BudgetExceeded,
    #[error("idempotency key required")]
    IdempotencyRequired,
    #[error("max_cost_usd required")]
    MaxCostRequired,
    #[error("session not ready")]
    NotReady,
    #[error("tunnel required")]
    TunnelRequired,
    #[error("tunnel auth required")]
    TunnelAuthRequired,
    #[error("journal error: {0}")]
    Journal(String),
}

impl From<BudgetError> for ClaudeError {
    fn from(err: BudgetError) -> Self {
        match err {
            BudgetError::Exceeded => ClaudeError::BudgetExceeded,
            BudgetError::ActualExceedsReservation => {
                ClaudeError::ProviderError("actual cost exceeded reservation".to_string())
            }
        }
    }
}

impl From<JournalError> for ClaudeError {
    fn from(err: JournalError) -> Self {
        ClaudeError::Journal(err.to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_container_config(policy: &ClaudePolicy) -> Result<ClaudeContainerConfig, ClaudeError> {
    let image = std::env::var("OPENAGENTS_CLAUDE_CONTAINER_IMAGE").map_err(|_| {
        ClaudeError::ProviderError("OPENAGENTS_CLAUDE_CONTAINER_IMAGE not set".to_string())
    })?;
    let command = std::env::var("OPENAGENTS_CLAUDE_CONTAINER_COMMAND").ok();
    let proxy_url = std::env::var("OPENAGENTS_CLAUDE_PROXY_URL").ok();
    let runtime = resolve_container_runtime()?;
    Ok(ClaudeContainerConfig {
        runtime,
        image,
        network_mode: policy.network_mode.clone(),
        proxy_url,
        command,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_container_runtime() -> Result<ClaudeContainerRuntime, ClaudeError> {
    if let Ok(value) = std::env::var("OPENAGENTS_CLAUDE_CONTAINER_RUNTIME") {
        let normalized = value.trim().to_lowercase();
        match normalized.as_str() {
            "apple" | "container" => {
                if apple_container_available() {
                    return Ok(ClaudeContainerRuntime::Apple);
                }
                return Err(ClaudeError::ProviderError(
                    "apple container runtime unavailable".to_string(),
                ));
            }
            "docker" => {
                if docker_available() {
                    return Ok(ClaudeContainerRuntime::Docker);
                }
                return Err(ClaudeError::ProviderError(
                    "docker runtime unavailable".to_string(),
                ));
            }
            "auto" | "" => {}
            _ => {
                return Err(ClaudeError::ProviderError(format!(
                    "invalid OPENAGENTS_CLAUDE_CONTAINER_RUNTIME: {}",
                    value
                )));
            }
        }
    }

    if apple_container_available() {
        Ok(ClaudeContainerRuntime::Apple)
    } else if docker_available() {
        Ok(ClaudeContainerRuntime::Docker)
    } else {
        Err(ClaudeError::ProviderError(
            "no container runtime available".to_string(),
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .arg("--format")
        .arg("{{.Server.Version}}")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
fn apple_container_available() -> bool {
    let major = macos_version_major().unwrap_or(0);
    if major < 26 {
        return false;
    }
    Command::new("container")
        .args(["system", "status"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(not(all(target_os = "macos", feature = "apple-container")))]
fn apple_container_available() -> bool {
    false
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
fn macos_version_major() -> Option<u32> {
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout);
    let mut parts = version.trim().split('.');
    let major = parts.next().unwrap_or("");
    let digits: String = major.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolConfig {
    min_workers: u32,
    max_workers: u32,
    idle_timeout_secs: u64,
    scale_up_threshold: f32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_workers: 0,
            max_workers: 0,
            idle_timeout_secs: 300,
            scale_up_threshold: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolMetrics {
    total_requests: u64,
    errors: u64,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            errors: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolStatus {
    total_workers: u32,
    idle_workers: u32,
    unhealthy_workers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkerStatus {
    Idle,
    Busy,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerMetrics {
    requests: u64,
    errors: u64,
}

impl Default for WorkerMetrics {
    fn default() -> Self {
        Self {
            requests: 0,
            errors: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerInfo {
    id: String,
    status: WorkerStatus,
    isolation: IsolationMode,
    sessions: u32,
    metrics: WorkerMetrics,
}

#[derive(Default)]
struct PoolState {
    config: PoolConfig,
    metrics: PoolMetrics,
    workers: HashMap<String, WorkerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProxyStatus {
    status: String,
}

impl Default for ProxyStatus {
    fn default() -> Self {
        Self {
            status: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProxyMetrics {
    requests: u64,
    blocked: u64,
}

impl Default for ProxyMetrics {
    fn default() -> Self {
        Self {
            requests: 0,
            blocked: 0,
        }
    }
}

#[derive(Default)]
struct ProxyState {
    status: ProxyStatus,
    allowlist: Vec<String>,
    metrics: ProxyMetrics,
}

#[derive(Clone, Copy)]
enum WorkerField {
    Status,
    Isolation,
    Sessions,
    Metrics,
}

fn matches_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == value;
    }
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let mut remainder = value;
    if !first.is_empty() {
        if let Some(stripped) = remainder.strip_prefix(first) {
            remainder = stripped;
        } else {
            return false;
        }
    }
    let mut tail = remainder;
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let Some(idx) = tail.find(part) {
            tail = &tail[idx + part.len()..];
        } else {
            return false;
        }
    }
    true
}

fn parse_pubkey(input: &str) -> FsResult<PublicKey> {
    if input.starts_with("npub") {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let entity = nostr::decode(input).map_err(|err| FsError::Other(err.to_string()))?;
            if let nostr::Nip19Entity::Pubkey(bytes) = entity {
                return Ok(PublicKey::new(bytes.to_vec()));
            }
            return Err(FsError::Other("invalid npub".to_string()));
        }
        #[cfg(target_arch = "wasm32")]
        {
            return Err(FsError::Other("bech32 pubkey unsupported".to_string()));
        }
    }
    let bytes = hex::decode(input).map_err(|err| FsError::Other(err.to_string()))?;
    Ok(PublicKey::new(bytes))
}

fn parse_signature(input: &str) -> FsResult<Signature> {
    let bytes = hex::decode(input)
        .or_else(|_| STANDARD.decode(input))
        .map_err(|err| FsError::Other(err.to_string()))?;
    Ok(Signature::new(bytes))
}

fn usd_to_microusd(cost: f64) -> u64 {
    if cost <= 0.0 {
        return 0;
    }
    (cost * 1_000_000.0).round() as u64
}

fn extract_text_from_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Object(map) => {
            if let Some(content) = map.get("content") {
                if let Some(text) = extract_text_from_value(content) {
                    return Some(text);
                }
            }
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
            None
        }
        serde_json::Value::Array(items) => {
            let mut out = String::new();
            for item in items {
                if let Some(text) = extract_text_from_value(item) {
                    out.push_str(&text);
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

fn extract_tool_blocks(value: &serde_json::Value) -> Vec<(String, serde_json::Value)> {
    fn block_from_value(value: &serde_json::Value) -> Option<(String, serde_json::Value)> {
        let map = value.as_object()?;
        let name = map
            .get("name")
            .or_else(|| map.get("tool_name"))
            .or_else(|| map.get("toolName"))
            .and_then(|v| v.as_str())?
            .to_string();
        let params = map
            .get("input")
            .or_else(|| map.get("params"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Some((name, params))
    }

    let mut blocks = Vec::new();
    if let Some(content) = value.get("content").and_then(|v| v.as_array()) {
        for item in content {
            if let Some(block) = block_from_value(item) {
                blocks.push(block);
            }
        }
    } else if let Some(items) = value.as_array() {
        for item in items {
            if let Some(block) = block_from_value(item) {
                blocks.push(block);
            }
        }
    } else if let Some(block) = block_from_value(value) {
        blocks.push(block);
    }
    blocks
}

fn extract_delta_from_event(event: &serde_json::Value) -> Option<String> {
    if let Some(delta) = event.get("delta") {
        if let Some(text) = extract_text_from_value(delta) {
            return Some(text);
        }
    }
    if let Some(text) = extract_text_from_value(event) {
        return Some(text);
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
mod providers {
    use super::*;
    use claude_agent_sdk::permissions::{CallbackPermissionHandler, PermissionRequest};
    use claude_agent_sdk::protocol::{
        PermissionResult, SdkMessage, SdkResultMessage, SdkSystemMessage,
    };
    use claude_agent_sdk::{ExecutableConfig, Query, QueryOptions, ToolsConfig};
    use futures::{SinkExt, StreamExt};
    use std::path::PathBuf;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    #[derive(Clone)]
    struct Executor {
        runtime: Arc<tokio::runtime::Runtime>,
    }

    impl Executor {
        fn new() -> Result<Self, ClaudeError> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|err| ClaudeError::ProviderError(err.to_string()))?;
            Ok(Self {
                runtime: Arc::new(runtime),
            })
        }

        fn spawn<F>(&self, fut: F)
        where
            F: std::future::Future<Output = ()> + Send + 'static,
        {
            self.runtime.spawn(fut);
        }
    }

    fn container_command(runtime: ClaudeContainerRuntime) -> &'static str {
        match runtime {
            ClaudeContainerRuntime::Apple => "container",
            ClaudeContainerRuntime::Docker => "docker",
        }
    }

    fn container_executable(
        config: &ClaudeContainerConfig,
        env: &HashMap<String, String>,
    ) -> ExecutableConfig {
        let mut args = Vec::new();
        args.push("run".to_string());
        args.push("--rm".to_string());
        args.push("-i".to_string());
        if matches!(config.network_mode, NetworkMode::None) {
            args.push("--network".to_string());
            args.push("none".to_string());
        }
        for (key, value) in env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }
        if let Some(proxy_url) = config.proxy_url.as_ref() {
            args.push("-e".to_string());
            args.push(format!("HTTPS_PROXY={}", proxy_url));
            args.push("-e".to_string());
            args.push(format!("HTTP_PROXY={}", proxy_url));
            args.push("-e".to_string());
            args.push("NODE_USE_ENV_PROXY=1".to_string());
        }
        args.push(config.image.clone());
        args.push(
            config
                .command
                .clone()
                .unwrap_or_else(|| "claude".to_string()),
        );
        ExecutableConfig {
            path: Some(PathBuf::from(container_command(config.runtime))),
            executable: None,
            executable_args: args,
        }
    }

    #[derive(Clone)]
    struct ProcessSession {
        state: Arc<RwLock<SessionState>>,
        prompt_tx: mpsc::Sender<String>,
        control_tx: mpsc::Sender<ControlCommand>,
        output_rx: Arc<Mutex<mpsc::Receiver<ClaudeChunk>>>,
        tool_log: Arc<Mutex<Vec<ToolLogEntry>>>,
        pending: Arc<Mutex<Option<PendingToolInfo>>>,
        pending_tx: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
        backend_session_id: Arc<Mutex<Option<String>>>,
        request: ClaudeRequest,
    }

    #[derive(Clone, Copy)]
    enum ControlCommand {
        Stop,
        Pause,
    }

    #[derive(Clone)]
    struct ProcessProvider {
        id: String,
        name: String,
        base_env: HashMap<String, String>,
        executor: Executor,
        executable: ExecutableConfig,
        sessions: Arc<RwLock<HashMap<String, ProcessSession>>>,
    }

    impl ProcessProvider {
        fn new(
            id: impl Into<String>,
            name: impl Into<String>,
            base_env: HashMap<String, String>,
        ) -> Result<Self, ClaudeError> {
            Ok(Self {
                id: id.into(),
                name: name.into(),
                base_env,
                executor: Executor::new()?,
                executable: ExecutableConfig::default(),
                sessions: Arc::new(RwLock::new(HashMap::new())),
            })
        }

        fn with_executable(mut self, executable: ExecutableConfig) -> Self {
            self.executable = executable;
            self
        }

        fn session(&self, session_id: &str) -> Result<ProcessSession, ClaudeError> {
            let guard = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            guard
                .get(session_id)
                .cloned()
                .ok_or(ClaudeError::SessionNotFound)
        }

        fn update_state(state: &Arc<RwLock<SessionState>>, next: SessionState) {
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            *guard = next;
        }

        fn map_result_usage(result: &SdkResultMessage) -> (Option<ClaudeUsage>, u64) {
            match result {
                SdkResultMessage::Success(success) => {
                    let usage = ClaudeUsage {
                        input_tokens: success.usage.input_tokens,
                        output_tokens: success.usage.output_tokens,
                        cache_read_tokens: success.usage.cache_read_input_tokens.unwrap_or(0),
                        cache_write_tokens: success.usage.cache_creation_input_tokens.unwrap_or(0),
                        total_tokens: success.usage.input_tokens + success.usage.output_tokens,
                    };
                    (Some(usage), usd_to_microusd(success.total_cost_usd))
                }
                SdkResultMessage::ErrorDuringExecution(error)
                | SdkResultMessage::ErrorMaxTurns(error)
                | SdkResultMessage::ErrorMaxBudget(error)
                | SdkResultMessage::ErrorMaxStructuredOutputRetries(error) => {
                    let usage = ClaudeUsage {
                        input_tokens: error.usage.input_tokens,
                        output_tokens: error.usage.output_tokens,
                        cache_read_tokens: error.usage.cache_read_input_tokens.unwrap_or(0),
                        cache_write_tokens: error.usage.cache_creation_input_tokens.unwrap_or(0),
                        total_tokens: error.usage.input_tokens + error.usage.output_tokens,
                    };
                    (Some(usage), usd_to_microusd(error.total_cost_usd))
                }
            }
        }

        fn spawn_session(
            &self,
            session_id: String,
            request: ClaudeRequest,
        ) -> Result<(), ClaudeError> {
            let (prompt_tx, prompt_rx) = mpsc::channel(128);
            let (control_tx, mut control_rx) = mpsc::channel(32);
            let (output_tx, output_rx) = mpsc::channel(256);
            let state = Arc::new(RwLock::new(SessionState::Creating {
                started_at: Timestamp::now(),
            }));
            let tool_log = Arc::new(Mutex::new(Vec::new()));
            let pending = Arc::new(Mutex::new(None));
            let pending_tx = Arc::new(Mutex::new(None));
            let response = Arc::new(Mutex::new(None));
            let usage = Arc::new(Mutex::new(None));
            let cost_usd = Arc::new(Mutex::new(0));
            let backend_session_id = Arc::new(Mutex::new(None));

            let session = ProcessSession {
                state: state.clone(),
                prompt_tx: prompt_tx.clone(),
                control_tx: control_tx.clone(),
                output_rx: Arc::new(Mutex::new(output_rx)),
                tool_log: tool_log.clone(),
                pending: pending.clone(),
                pending_tx: pending_tx.clone(),
                backend_session_id: backend_session_id.clone(),
                request: request.clone(),
            };

            self.sessions
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(session_id.clone(), session.clone());

            let executor = self.executor.clone();
            let provider_id = self.id.clone();
            let base_env = self.base_env.clone();
            let executable = self.executable.clone();
            executor.spawn(async move {
                let handler = build_permission_handler(
                    request.internal.tool_policy.clone(),
                    state.clone(),
                    tool_log.clone(),
                    pending.clone(),
                    pending_tx.clone(),
                );

                let mut options = QueryOptions::new();
                options.model = Some(request.model.clone());
                if let Some(system_prompt) = request.system_prompt.as_ref() {
                    options.system_prompt = Some(claude_agent_sdk::options::SystemPromptConfig::Custom(
                        system_prompt.clone(),
                    ));
                }
                if let Some(max_cost) = request.max_cost_usd {
                    options.max_budget_usd = Some(max_cost as f64 / 1_000_000.0);
                }
                if !request.tools.is_empty() {
                    let tools: Vec<String> = request.tools.iter().map(|t| t.name.clone()).collect();
                    options.tools = Some(ToolsConfig::list(tools));
                }
                if !request.internal.tool_policy.allowed.is_empty() {
                    options.allowed_tools = Some(request.internal.tool_policy.allowed.clone());
                }
                if !request.internal.tool_policy.blocked.is_empty() {
                    options.disallowed_tools = Some(request.internal.tool_policy.blocked.clone());
                }
                options.include_partial_messages = true;
                if let Some(resume_backend_id) = request.internal.resume_backend_id.clone() {
                    options.resume = Some(resume_backend_id);
                    options.fork_session = request.internal.fork;
                }
                options.env = Some(base_env);
                let executable = request
                    .internal
                    .executable
                    .clone()
                    .unwrap_or(executable);
                options.executable = executable;
                let prompt = request.initial_prompt.clone().unwrap_or_default();

                let mut query = match Query::new(prompt, options, Some(handler)).await {
                    Ok(query) => query,
                    Err(err) => {
                        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        };
                        return;
                    }
                };

                let _ = query.stream_input(ReceiverStream::new(prompt_rx)).await;

                let mut last_tool_name: Option<String> = None;

                loop {
                    tokio::select! {
                        Some(cmd) = control_rx.recv() => {
                            match cmd {
                                ControlCommand::Stop => {
                                    let _ = query.interrupt().await;
                                    let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Failed { error: "stopped".to_string(), at: Timestamp::now() };
                                    break;
                                }
                                ControlCommand::Pause => {
                                    let _ = query.interrupt().await;
                                    let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Idle { last_response_at: Timestamp::now(), response: response.lock().unwrap_or_else(|e| e.into_inner()).clone(), usage: usage.lock().unwrap_or_else(|e| e.into_inner()).clone(), cost_usd: *cost_usd.lock().unwrap_or_else(|e| e.into_inner()) };
                                }
                            }
                        }
                        msg = query.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(SdkMessage::System(SdkSystemMessage::Init(init))) => {
                                    *backend_session_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(init.session_id.clone());
                                    ProcessProvider::update_state(&state, SessionState::Ready { created_at: Timestamp::now() });
                                }
                                Ok(SdkMessage::StreamEvent(event)) => {
                                    if let Some(delta) = extract_delta_from_event(&event.event) {
                                        let chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type: ChunkType::Text, delta: Some(delta), tool: None, usage: None };
                                        let _ = output_tx.send(chunk).await;
                                    }
                                }
                                Ok(SdkMessage::Assistant(assistant)) => {
                                    if let Some(text) = extract_text_from_value(&assistant.message) {
                                        {
                                            let mut guard = response.lock().unwrap_or_else(|e| e.into_inner());
                                            *guard = Some(text.clone());
                                        }
                                        let chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type: ChunkType::Text, delta: Some(text), tool: None, usage: None };
                                        let _ = output_tx.send(chunk).await;
                                    }
                                    for (tool_name, params) in extract_tool_blocks(&assistant.message) {
                                        last_tool_name = Some(tool_name.clone());
                                        let chunk = ClaudeChunk {
                                            session_id: session_id.clone(),
                                            chunk_type: ChunkType::ToolStart,
                                            delta: None,
                                            tool: Some(ToolChunk {
                                                name: tool_name,
                                                params: Some(params),
                                                result: None,
                                                error: None,
                                            }),
                                            usage: None,
                                        };
                                        let _ = output_tx.send(chunk).await;
                                    }
                                    ProcessProvider::update_state(&state, SessionState::Working { started_at: Timestamp::now(), current_tool: None });
                                }
                                Ok(SdkMessage::User(user_msg)) => {
                                    if let Some(tool_result) = user_msg.tool_use_result.as_ref() {
                                        let tool_name = tool_result
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .or_else(|| tool_result.get("tool_name").and_then(|v| v.as_str()))
                                            .map(|name| name.to_string())
                                            .or_else(|| last_tool_name.clone())
                                            .unwrap_or_else(|| "tool".to_string());
                                        let tool_error = tool_result
                                            .get("error")
                                            .and_then(|v| v.as_str())
                                            .map(|err| err.to_string());
                                        let chunk = ClaudeChunk {
                                            session_id: session_id.clone(),
                                            chunk_type: ChunkType::ToolDone,
                                            delta: None,
                                            tool: Some(ToolChunk {
                                                name: tool_name.clone(),
                                                params: None,
                                                result: Some(tool_result.clone()),
                                                error: tool_error,
                                            }),
                                            usage: None,
                                        };
                                        let _ = output_tx.send(chunk).await;
                                        last_tool_name = Some(tool_name);
                                    }
                                    ProcessProvider::update_state(&state, SessionState::Working { started_at: Timestamp::now(), current_tool: None });
                                }
                                Ok(SdkMessage::ToolProgress(progress)) => {
                                    last_tool_name = Some(progress.tool_name.clone());
                                    let chunk = ClaudeChunk {
                                        session_id: session_id.clone(),
                                        chunk_type: ChunkType::ToolStart,
                                        delta: None,
                                        tool: Some(ToolChunk { name: progress.tool_name.clone(), params: None, result: None, error: None }),
                                        usage: None,
                                    };
                                    let _ = output_tx.send(chunk).await;
                                }
                                Ok(SdkMessage::Result(result)) => {
                                    let (usage_value, cost_value) = ProcessProvider::map_result_usage(&result);
                                    *usage.lock().unwrap_or_else(|e| e.into_inner()) = usage_value.clone();
                                    *cost_usd.lock().unwrap_or_else(|e| e.into_inner()) = cost_value;
                                    let (status, response_text) = match &result {
                                        SdkResultMessage::Success(success) => {
                                            if success.is_error {
                                                (ClaudeSessionStatus::Failed { error: "execution error".to_string() }, None)
                                            } else {
                                                (ClaudeSessionStatus::Complete, Some(success.result.clone()))
                                            }
                                        }
                                        SdkResultMessage::ErrorDuringExecution(error)
                                        | SdkResultMessage::ErrorMaxTurns(error)
                                        | SdkResultMessage::ErrorMaxBudget(error)
                                        | SdkResultMessage::ErrorMaxStructuredOutputRetries(error) => {
                                            let message = if error.errors.is_empty() {
                                                "execution error".to_string()
                                            } else {
                                                error.errors.join("; ")
                                            };
                                            (ClaudeSessionStatus::Failed { error: message }, None)
                                        }
                                    };
                                    if let Some(text) = response_text.clone() {
                                        *response.lock().unwrap_or_else(|e| e.into_inner()) = Some(text.clone());
                                    }
                                    let response_value = ClaudeResponse {
                                        session_id: session_id.clone(),
                                        status: status.clone(),
                                        response: response_text.clone(),
                                        usage: usage_value.clone(),
                                        cost_usd: cost_value,
                                        reserved_usd: 0,
                                        provider_id: provider_id.clone(),
                                        model: request.model.clone(),
                                        tunnel_endpoint: request.tunnel_endpoint.clone(),
                                    };
                                    ProcessProvider::update_state(&state, SessionState::Complete(response_value));
                                    let chunk_type = match status {
                                        ClaudeSessionStatus::Failed { .. } => ChunkType::Error,
                                        _ => ChunkType::Done,
                                    };
                                    let done_chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type, delta: response_text, tool: None, usage: usage_value };
                                    let _ = output_tx.send(done_chunk).await;
                                }
                                Ok(_) => {}
                                Err(err) => {
                                    ProcessProvider::update_state(&state, SessionState::Failed { error: err.to_string(), at: Timestamp::now() });
                                    let err_chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type: ChunkType::Error, delta: Some(err.to_string()), tool: None, usage: None };
                                    let _ = output_tx.send(err_chunk).await;
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        }
    }

    fn build_permission_handler(
        policy: ToolPolicy,
        state: Arc<RwLock<SessionState>>,
        tool_log: Arc<Mutex<Vec<ToolLogEntry>>>,
        pending: Arc<Mutex<Option<PendingToolInfo>>>,
        pending_tx: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
    ) -> Arc<dyn claude_agent_sdk::permissions::PermissionHandler> {
        let handler = move |request: PermissionRequest| {
            let policy = policy.clone();
            let state = state.clone();
            let tool_log = tool_log.clone();
            let pending = pending.clone();
            let pending_tx = pending_tx.clone();
            async move {
                let tool_name = request.tool_name.clone();
                let params = request.input.clone();

                if !policy.allowed.is_empty() && !policy.allowed.iter().any(|t| t == &tool_name) {
                    tool_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(ToolLogEntry {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params,
                            approved: Some(false),
                            error: Some("tool not allowed".to_string()),
                            timestamp: Timestamp::now(),
                        });
                    return Ok(PermissionResult::deny("tool not allowed"));
                }

                if policy.blocked.iter().any(|t| t == &tool_name) {
                    tool_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(ToolLogEntry {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params,
                            approved: Some(false),
                            error: Some("tool blocked".to_string()),
                            timestamp: Timestamp::now(),
                        });
                    return Ok(PermissionResult::deny("tool blocked"));
                }

                if policy.autonomy == ClaudeSessionAutonomy::ReadOnly {
                    tool_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(ToolLogEntry {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params,
                            approved: Some(false),
                            error: Some("read-only".to_string()),
                            timestamp: Timestamp::now(),
                        });
                    return Ok(PermissionResult::deny("read-only"));
                }

                let requires_approval =
                    matches!(policy.autonomy, ClaudeSessionAutonomy::Restricted)
                        || (policy.autonomy == ClaudeSessionAutonomy::Supervised
                            && policy.approval_required.iter().any(|t| t == &tool_name));

                if requires_approval {
                    let (tx, rx) = oneshot::channel();
                    {
                        let mut guard = pending.lock().unwrap_or_else(|e| e.into_inner());
                        if guard.is_some() {
                            return Ok(PermissionResult::deny("tool approval already pending"));
                        }
                        *guard = Some(PendingToolInfo {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params: params.clone(),
                            requested_at: Timestamp::now(),
                        });
                        *pending_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
                    }
                    {
                        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::PendingApproval {
                            tool: tool_name.clone(),
                            params: params.clone(),
                            since: Timestamp::now(),
                        };
                    }
                    tool_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(ToolLogEntry {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params: params.clone(),
                            approved: None,
                            error: None,
                            timestamp: Timestamp::now(),
                        });

                    let approved = rx.await.unwrap_or(false);
                    tool_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(ToolLogEntry {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params,
                            approved: Some(approved),
                            error: if approved {
                                None
                            } else {
                                Some("denied".to_string())
                            },
                            timestamp: Timestamp::now(),
                        });
                    {
                        let mut guard = pending.lock().unwrap_or_else(|e| e.into_inner());
                        *guard = None;
                    }
                    {
                        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::Working {
                            started_at: Timestamp::now(),
                            current_tool: Some(tool_name.clone()),
                        };
                    }

                    if approved {
                        Ok(PermissionResult::allow(request.input.clone()))
                    } else {
                        Ok(PermissionResult::deny_and_interrupt("tool denied"))
                    }
                } else {
                    tool_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(ToolLogEntry {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params,
                            approved: Some(true),
                            error: None,
                            timestamp: Timestamp::now(),
                        });
                    Ok(PermissionResult::allow(request.input.clone()))
                }
            }
        };
        Arc::new(CallbackPermissionHandler::new(handler))
    }

    #[derive(Clone)]
    pub struct LocalProvider {
        inner: ProcessProvider,
    }

    impl LocalProvider {
        pub fn new() -> Result<Self, ClaudeError> {
            let inner = ProcessProvider::new("local", "Local Claude", HashMap::new())?;
            Ok(Self { inner })
        }

        pub fn with_executable(mut self, executable: ExecutableConfig) -> Self {
            self.inner = self.inner.with_executable(executable);
            self
        }
    }

    impl ClaudeProvider for LocalProvider {
        fn id(&self) -> &str {
            &self.inner.id
        }

        fn info(&self) -> ClaudeProviderInfo {
            ClaudeProviderInfo {
                id: self.inner.id.clone(),
                name: self.inner.name.clone(),
                models: Vec::new(),
                capabilities: ClaudeCapabilities {
                    streaming: true,
                    resume: true,
                    fork: true,
                    tools: true,
                    vision: false,
                },
                pricing: None,
                status: ClaudeProviderStatus::Available,
            }
        }

        fn is_available(&self) -> bool {
            true
        }

        fn supports_model(&self, _model: &str) -> bool {
            true
        }

        fn create_session(&self, mut request: ClaudeRequest) -> Result<String, ClaudeError> {
            if let Some(container) = request.internal.container.clone() {
                request.internal.executable =
                    Some(container_executable(&container, &self.inner.base_env));
            }
            let session_id = uuid::Uuid::new_v4().to_string();
            if let Some(resume_id) = request.resume_session_id.as_ref() {
                let session = self.inner.session(resume_id)?;
                let backend = session
                    .backend_session_id
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                    .ok_or_else(|| {
                        ClaudeError::ProviderError("resume session not ready".to_string())
                    })?;
                request.internal.resume_backend_id = Some(backend);
            }
            self.inner.spawn_session(session_id.clone(), request)?;
            Ok(session_id)
        }

        fn get_session(&self, session_id: &str) -> Option<SessionState> {
            let session = self.inner.session(session_id).ok()?;
            Some(
                session
                    .state
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            )
        }

        fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.prompt_tx.try_send(prompt.to_string());
            ProcessProvider::update_state(
                &session.state,
                SessionState::Working {
                    started_at: Timestamp::now(),
                    current_tool: None,
                },
            );
            Ok(())
        }

        fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
            Ok(rx.try_recv().ok())
        }

        fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let tx = session
                .pending_tx
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some(tx) = tx {
                let _ = tx.send(approved);
                Ok(())
            } else {
                Err(ClaudeError::ProviderError("no pending tool".to_string()))
            }
        }

        fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut request = session.request.clone();
            request.resume_session_id = Some(session_id.to_string());
            request.internal.fork = true;
            self.create_session(request)
        }

        fn stop(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Stop);
            Ok(())
        }

        fn pause(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Pause);
            Ok(())
        }

        fn resume(&self, _session_id: &str) -> Result<(), ClaudeError> {
            Ok(())
        }

        fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>> {
            let session = self.inner.session(session_id).ok()?;
            Some(
                session
                    .tool_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            )
        }

        fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo> {
            let session = self.inner.session(session_id).ok()?;
            session
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
    }

    #[derive(Clone)]
    pub struct CloudProvider {
        inner: ProcessProvider,
        api_key: Option<String>,
    }

    impl CloudProvider {
        pub fn new(api_key: Option<String>) -> Result<Self, ClaudeError> {
            let mut env = HashMap::new();
            if let Some(ref key) = api_key {
                env.insert("ANTHROPIC_API_KEY".to_string(), key.clone());
            }
            let inner = ProcessProvider::new("cloud", "Anthropic Cloud", env)?;
            Ok(Self { inner, api_key })
        }

        pub fn from_env() -> Result<Self, ClaudeError> {
            let key = std::env::var("ANTHROPIC_API_KEY").ok();
            Self::new(key)
        }
    }

    impl ClaudeProvider for CloudProvider {
        fn id(&self) -> &str {
            &self.inner.id
        }

        fn info(&self) -> ClaudeProviderInfo {
            ClaudeProviderInfo {
                id: self.inner.id.clone(),
                name: self.inner.name.clone(),
                models: Vec::new(),
                capabilities: ClaudeCapabilities {
                    streaming: true,
                    resume: true,
                    fork: true,
                    tools: true,
                    vision: false,
                },
                pricing: None,
                status: if self.api_key.is_some() {
                    ClaudeProviderStatus::Available
                } else {
                    ClaudeProviderStatus::Unavailable {
                        reason: "missing ANTHROPIC_API_KEY".to_string(),
                    }
                },
            }
        }

        fn is_available(&self) -> bool {
            self.api_key.is_some()
        }

        fn supports_model(&self, _model: &str) -> bool {
            true
        }

        fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError> {
            if self.api_key.is_none() {
                return Err(ClaudeError::ProviderError(
                    "missing ANTHROPIC_API_KEY".to_string(),
                ));
            }
            let mut request = request;
            if let Some(container) = request.internal.container.clone() {
                request.internal.executable =
                    Some(container_executable(&container, &self.inner.base_env));
            }
            if let Some(resume_id) = request.resume_session_id.as_ref() {
                let session = self.inner.session(resume_id)?;
                let backend = session
                    .backend_session_id
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                    .ok_or_else(|| {
                        ClaudeError::ProviderError("resume session not ready".to_string())
                    })?;
                request.internal.resume_backend_id = Some(backend);
            }
            let session_id = uuid::Uuid::new_v4().to_string();
            self.inner.spawn_session(session_id.clone(), request)?;
            Ok(session_id)
        }

        fn get_session(&self, session_id: &str) -> Option<SessionState> {
            let session = self.inner.session(session_id).ok()?;
            Some(
                session
                    .state
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            )
        }

        fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.prompt_tx.try_send(prompt.to_string());
            ProcessProvider::update_state(
                &session.state,
                SessionState::Working {
                    started_at: Timestamp::now(),
                    current_tool: None,
                },
            );
            Ok(())
        }

        fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
            Ok(rx.try_recv().ok())
        }

        fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let tx = session
                .pending_tx
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some(tx) = tx {
                let _ = tx.send(approved);
                Ok(())
            } else {
                Err(ClaudeError::ProviderError("no pending tool".to_string()))
            }
        }

        fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut request = session.request.clone();
            request.resume_session_id = Some(session_id.to_string());
            request.internal.fork = true;
            self.create_session(request)
        }

        fn stop(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Stop);
            Ok(())
        }

        fn pause(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Pause);
            Ok(())
        }

        fn resume(&self, _session_id: &str) -> Result<(), ClaudeError> {
            Ok(())
        }

        fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>> {
            let session = self.inner.session(session_id).ok()?;
            Some(
                session
                    .tool_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            )
        }

        fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo> {
            let session = self.inner.session(session_id).ok()?;
            session
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum TunnelMessage {
        Auth {
            response: TunnelAuthResponse,
        },
        CreateSession {
            request: ClaudeRequest,
            session_id: String,
        },
        SessionCreated {
            session_id: String,
        },
        Prompt {
            session_id: String,
            content: String,
        },
        Chunk(ClaudeChunk),
        ToolApproval {
            session_id: String,
            tool: String,
            params: serde_json::Value,
        },
        ToolApprovalResponse {
            session_id: String,
            approved: bool,
        },
        Stop {
            session_id: String,
        },
        Pause {
            session_id: String,
        },
        Resume {
            session_id: String,
        },
        Error {
            session_id: String,
            error: String,
        },
    }

    #[derive(Clone)]
    struct TunnelSession {
        state: Arc<RwLock<SessionState>>,
        output_rx: Arc<Mutex<mpsc::Receiver<ClaudeChunk>>>,
        sender: mpsc::Sender<TunnelMessage>,
        tool_log: Arc<Mutex<Vec<ToolLogEntry>>>,
        pending: Arc<Mutex<Option<PendingToolInfo>>>,
        current_tool_id: Arc<Mutex<Option<String>>>,
        request: ClaudeRequest,
    }

    #[derive(Clone)]
    pub struct TunnelProvider {
        endpoints: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
        executor: Executor,
        sessions: Arc<RwLock<HashMap<String, TunnelSession>>>,
    }

    impl TunnelProvider {
        pub fn new(
            endpoints: Arc<RwLock<Vec<TunnelEndpoint>>>,
            auth_state: Arc<RwLock<TunnelAuthState>>,
        ) -> Result<Self, ClaudeError> {
            Ok(Self {
                endpoints,
                auth_state,
                executor: Executor::new()?,
                sessions: Arc::new(RwLock::new(HashMap::new())),
            })
        }

        fn session(&self, session_id: &str) -> Result<TunnelSession, ClaudeError> {
            let guard = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            guard
                .get(session_id)
                .cloned()
                .ok_or(ClaudeError::SessionNotFound)
        }

        fn endpoint_for(&self, id: &str) -> Result<TunnelEndpoint, ClaudeError> {
            let guard = self.endpoints.read().unwrap_or_else(|e| e.into_inner());
            guard
                .iter()
                .find(|e| e.id == id)
                .cloned()
                .ok_or_else(|| ClaudeError::ProviderError("tunnel not found".to_string()))
        }

        fn auth_for(&self, tunnel_id: &str) -> Option<TunnelAuthResponse> {
            let now = Timestamp::now();
            let mut auth = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
            let challenge = auth.challenges.get(tunnel_id)?;
            if challenge.expires_at.as_millis() <= now.as_millis() {
                auth.responses.remove(tunnel_id);
                return None;
            }
            let response = auth.responses.get(tunnel_id)?;
            if response.challenge != challenge.challenge {
                auth.responses.remove(tunnel_id);
                return None;
            }
            Some(response.clone())
        }

        fn spawn_connection(
            &self,
            endpoint: TunnelEndpoint,
            session_id: String,
            request: ClaudeRequest,
        ) -> Result<TunnelSession, ClaudeError> {
            let (sender_tx, mut sender_rx) = mpsc::channel(128);
            let (output_tx, output_rx) = mpsc::channel(256);
            let state = Arc::new(RwLock::new(SessionState::Creating {
                started_at: Timestamp::now(),
            }));
            let tool_log = Arc::new(Mutex::new(Vec::new()));
            let pending = Arc::new(Mutex::new(None));
            let current_tool_id = Arc::new(Mutex::new(None));
            let response = Arc::new(Mutex::new(String::new()));
            let usage = Arc::new(Mutex::new(None));
            let auth = self.auth_for(&endpoint.id);

            let executor = self.executor.clone();
            let session_id_clone = session_id.clone();
            let state_clone = state.clone();
            let pending_clone = pending.clone();
            let tool_log_clone = tool_log.clone();
            let current_tool_id_clone = current_tool_id.clone();
            let response_clone = response.clone();
            let usage_clone = usage.clone();
            let request_clone = request.clone();
            executor.spawn(async move {
                let (ws_stream, _) = match tokio_tungstenite::connect_async(&endpoint.url).await {
                    Ok(result) => result,
                    Err(err) => {
                        let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        };
                        return;
                    }
                };
                let (mut write, mut read) = ws_stream.split();

                if let Some(response) = auth.clone() {
                    let auth_msg = TunnelMessage::Auth { response };
                    let _ = write
                        .send(WsMessage::Text(
                            serde_json::to_string(&auth_msg).unwrap_or_default(),
                        ))
                        .await;
                }

                let create_msg = TunnelMessage::CreateSession {
                    request: request_clone.clone(),
                    session_id: session_id_clone.clone(),
                };
                let _ = write
                    .send(WsMessage::Text(
                        serde_json::to_string(&create_msg).unwrap_or_default(),
                    ))
                    .await;

                let write_task = tokio::spawn(async move {
                    while let Some(msg) = sender_rx.recv().await {
                        let payload = serde_json::to_string(&msg).unwrap_or_default();
                        if write.send(WsMessage::Text(payload)).await.is_err() {
                            break;
                        }
                    }
                });

                while let Some(msg) = read.next().await {
                    let msg = match msg {
                        Ok(WsMessage::Text(text)) => {
                            serde_json::from_str::<TunnelMessage>(&text).ok()
                        }
                        Ok(WsMessage::Binary(bin)) => {
                            serde_json::from_slice::<TunnelMessage>(&bin).ok()
                        }
                        _ => None,
                    };
                    let Some(msg) = msg else {
                        continue;
                    };
                    match msg {
                        TunnelMessage::SessionCreated { .. } => {
                            let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                            *guard = SessionState::Ready {
                                created_at: Timestamp::now(),
                            };
                        }
                        TunnelMessage::Chunk(chunk) => {
                            let chunk_type = chunk.chunk_type.clone();
                            let tool = chunk.tool.clone();
                            let delta = chunk.delta.clone();
                            let usage_value = chunk.usage.clone();

                            if let Some(delta) = delta.as_ref() {
                                if matches!(chunk_type, ChunkType::Text | ChunkType::Done) {
                                    let mut guard =
                                        response_clone.lock().unwrap_or_else(|e| e.into_inner());
                                    guard.push_str(delta);
                                }
                            }
                            if let Some(usage) = usage_value.clone() {
                                *usage_clone.lock().unwrap_or_else(|e| e.into_inner()) =
                                    Some(usage);
                            }

                            match chunk_type {
                                ChunkType::Text => {
                                    let mut guard =
                                        state_clone.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Working {
                                        started_at: Timestamp::now(),
                                        current_tool: None,
                                    };
                                }
                                ChunkType::ToolStart => {
                                    if let Some(tool) = tool.clone() {
                                        let tool_use_id = {
                                            let mut guard = current_tool_id_clone
                                                .lock()
                                                .unwrap_or_else(|e| e.into_inner());
                                            if let Some(existing) = guard.clone() {
                                                existing
                                            } else {
                                                let id = uuid::Uuid::new_v4().to_string();
                                                *guard = Some(id.clone());
                                                id
                                            }
                                        };
                                        tool_log_clone
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner())
                                            .push(ToolLogEntry {
                                                tool_use_id,
                                                tool: tool.name.clone(),
                                                params: tool
                                                    .params
                                                    .clone()
                                                    .unwrap_or(serde_json::Value::Null),
                                                approved: None,
                                                error: None,
                                                timestamp: Timestamp::now(),
                                            });
                                        let mut guard =
                                            state_clone.write().unwrap_or_else(|e| e.into_inner());
                                        *guard = SessionState::Working {
                                            started_at: Timestamp::now(),
                                            current_tool: Some(tool.name),
                                        };
                                    }
                                }
                                ChunkType::ToolDone => {
                                    if let Some(tool) = tool.clone() {
                                        let tool_use_id = {
                                            let mut guard = current_tool_id_clone
                                                .lock()
                                                .unwrap_or_else(|e| e.into_inner());
                                            guard
                                                .take()
                                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
                                        };
                                        tool_log_clone
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner())
                                            .push(ToolLogEntry {
                                                tool_use_id,
                                                tool: tool.name.clone(),
                                                params: tool
                                                    .params
                                                    .clone()
                                                    .unwrap_or(serde_json::Value::Null),
                                                approved: Some(tool.error.is_none()),
                                                error: tool.error.clone(),
                                                timestamp: Timestamp::now(),
                                            });
                                        let mut guard =
                                            state_clone.write().unwrap_or_else(|e| e.into_inner());
                                        *guard = SessionState::Working {
                                            started_at: Timestamp::now(),
                                            current_tool: None,
                                        };
                                    }
                                }
                                ChunkType::Done => {
                                    let response_text = {
                                        let guard = response_clone
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner());
                                        if guard.is_empty() {
                                            None
                                        } else {
                                            Some(guard.clone())
                                        }
                                    };
                                    let usage_value = usage_clone
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner())
                                        .clone();
                                    let cost_usd = request_clone.max_cost_usd.unwrap_or(0);
                                    let response = ClaudeResponse {
                                        session_id: session_id_clone.clone(),
                                        status: ClaudeSessionStatus::Complete,
                                        response: response_text,
                                        usage: usage_value,
                                        cost_usd,
                                        reserved_usd: cost_usd,
                                        provider_id: "tunnel".to_string(),
                                        model: request_clone.model.clone(),
                                        tunnel_endpoint: request_clone.tunnel_endpoint.clone(),
                                    };
                                    let mut guard =
                                        state_clone.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Complete(response);
                                    *pending_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                                    *current_tool_id_clone
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner()) = None;
                                }
                                ChunkType::Error => {
                                    let error = delta.unwrap_or_else(|| "error".to_string());
                                    let mut guard =
                                        state_clone.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Failed {
                                        error,
                                        at: Timestamp::now(),
                                    };
                                    *pending_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                                    *current_tool_id_clone
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner()) = None;
                                }
                                _ => {}
                            }

                            let _ = output_tx.send(chunk).await;
                        }
                        TunnelMessage::ToolApproval { tool, params, .. } => {
                            let tool_use_id = uuid::Uuid::new_v4().to_string();
                            let info = PendingToolInfo {
                                tool_use_id: tool_use_id.clone(),
                                tool: tool.clone(),
                                params: params.clone(),
                                requested_at: Timestamp::now(),
                            };
                            *pending_clone.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(info.clone());
                            let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                            *guard = SessionState::PendingApproval {
                                tool,
                                params,
                                since: Timestamp::now(),
                            };
                            tool_log_clone
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .push(ToolLogEntry {
                                    tool_use_id,
                                    tool: info.tool.clone(),
                                    params: info.params.clone(),
                                    approved: None,
                                    error: None,
                                    timestamp: Timestamp::now(),
                                });
                        }
                        TunnelMessage::Error { error, .. } => {
                            let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                            *guard = SessionState::Failed {
                                error,
                                at: Timestamp::now(),
                            };
                        }
                        _ => {}
                    }
                }
                write_task.abort();
            });

            Ok(TunnelSession {
                state,
                output_rx: Arc::new(Mutex::new(output_rx)),
                sender: sender_tx,
                tool_log,
                pending,
                current_tool_id,
                request,
            })
        }
    }

    impl ClaudeProvider for TunnelProvider {
        fn id(&self) -> &str {
            "tunnel"
        }

        fn info(&self) -> ClaudeProviderInfo {
            ClaudeProviderInfo {
                id: "tunnel".to_string(),
                name: "Tunnel".to_string(),
                models: Vec::new(),
                capabilities: ClaudeCapabilities {
                    streaming: true,
                    resume: true,
                    fork: true,
                    tools: true,
                    vision: true,
                },
                pricing: None,
                status: ClaudeProviderStatus::Available,
            }
        }

        fn is_available(&self) -> bool {
            !self
                .endpoints
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
        }

        fn supports_model(&self, _model: &str) -> bool {
            true
        }

        fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError> {
            let endpoint_id = request
                .tunnel_endpoint
                .as_ref()
                .ok_or(ClaudeError::TunnelRequired)?;
            let endpoint = self.endpoint_for(endpoint_id)?;
            if matches!(endpoint.auth, TunnelAuth::Nostr { .. })
                && self.auth_for(&endpoint.id).is_none()
            {
                return Err(ClaudeError::TunnelAuthRequired);
            }
            let session_id = uuid::Uuid::new_v4().to_string();
            let session = self.spawn_connection(endpoint, session_id.clone(), request)?;
            self.sessions
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(session_id.clone(), session);
            Ok(session_id)
        }

        fn get_session(&self, session_id: &str) -> Option<SessionState> {
            let session = self.session(session_id).ok()?;
            Some(
                session
                    .state
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            )
        }

        fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::Prompt {
                session_id: session_id.to_string(),
                content: prompt.to_string(),
            });
            let mut guard = session.state.write().unwrap_or_else(|e| e.into_inner());
            *guard = SessionState::Working {
                started_at: Timestamp::now(),
                current_tool: None,
            };
            Ok(())
        }

        fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError> {
            let session = self.session(session_id)?;
            let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
            Ok(rx.try_recv().ok())
        }

        fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session
                .sender
                .try_send(TunnelMessage::ToolApprovalResponse {
                    session_id: session_id.to_string(),
                    approved,
                });
            if let Some(info) = session
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                session
                    .tool_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(ToolLogEntry {
                        tool_use_id: info.tool_use_id.clone(),
                        tool: info.tool.clone(),
                        params: info.params.clone(),
                        approved: Some(approved),
                        error: if approved {
                            None
                        } else {
                            Some("denied".to_string())
                        },
                        timestamp: Timestamp::now(),
                    });
                if approved {
                    *session
                        .current_tool_id
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(info.tool_use_id);
                    let mut guard = session.state.write().unwrap_or_else(|e| e.into_inner());
                    *guard = SessionState::Working {
                        started_at: Timestamp::now(),
                        current_tool: Some(info.tool),
                    };
                } else {
                    let mut guard = session.state.write().unwrap_or_else(|e| e.into_inner());
                    *guard = SessionState::Failed {
                        error: "tool denied".to_string(),
                        at: Timestamp::now(),
                    };
                }
            }
            Ok(())
        }

        fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError> {
            let session = self.session(session_id)?;
            let mut request = session.request.clone();
            request.resume_session_id = Some(session_id.to_string());
            request.internal.fork = true;
            self.create_session(request)
        }

        fn stop(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::Stop {
                session_id: session_id.to_string(),
            });
            Ok(())
        }

        fn pause(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::Pause {
                session_id: session_id.to_string(),
            });
            Ok(())
        }

        fn resume(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::Resume {
                session_id: session_id.to_string(),
            });
            Ok(())
        }

        fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>> {
            let session = self.session(session_id).ok()?;
            Some(
                session
                    .tool_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            )
        }

        fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo> {
            let session = self.session(session_id).ok()?;
            session
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use providers::{CloudProvider, LocalProvider, TunnelProvider};
