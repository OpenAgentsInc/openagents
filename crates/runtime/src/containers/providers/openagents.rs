/// OpenAgents API-backed container provider (cloudflare/daytona).
pub struct OpenAgentsContainerProvider {
    provider_id: String,
    name: String,
    api: Arc<dyn OpenAgentsApiClient>,
    auth: Arc<OpenAgentsAuth>,
    session_cursors: Arc<Mutex<HashMap<String, String>>>,
    exec_cursors: Arc<Mutex<HashMap<String, String>>>,
}

impl OpenAgentsContainerProvider {
    pub fn new(
        provider_id: impl Into<String>,
        name: impl Into<String>,
        api: Arc<dyn OpenAgentsApiClient>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            name: name.into(),
            api,
            auth,
            session_cursors: Arc::new(Mutex::new(HashMap::new())),
            exec_cursors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn cloudflare(api: Arc<dyn OpenAgentsApiClient>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("cloudflare", "Cloudflare Containers", api, auth)
    }

    pub fn daytona(api: Arc<dyn OpenAgentsApiClient>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("daytona", "Daytona Cloud Sandbox", api, auth)
    }

    fn require_token(&self) -> Result<String, ContainerError> {
        self.auth.token().ok_or(ContainerError::AuthRequired {
            provider: self.provider_id.clone(),
            message: "OpenAgents API token required".to_string(),
        })
    }
}

impl ContainerProvider for OpenAgentsContainerProvider {
    fn id(&self) -> &str {
        &self.provider_id
    }

    fn info(&self) -> ContainerProviderInfo {
        let token = self.auth.token();
        match self.api.provider_info(&self.provider_id, token.as_deref()) {
            Ok(info) => info,
            Err(err) => unavailable_provider_info(
                &self.provider_id,
                &self.name,
                format!("OpenAgents API error: {}", err),
            ),
        }
    }

    fn is_available(&self) -> bool {
        matches!(
            self.info().status,
            ProviderStatus::Available | ProviderStatus::Degraded { .. }
        )
    }

    fn requires_openagents_auth(&self) -> bool {
        true
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        self.api
            .submit_container(&self.provider_id, &request, &token)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let token = self.require_token().ok()?;
        match self.api.session_state(session_id, &token) {
            Ok(state) => Some(state),
            Err(ContainerError::SessionNotFound) => None,
            Err(err) => Some(SessionState::Failed {
                error: err.to_string(),
                at: Timestamp::now(),
            }),
        }
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        self.api.submit_exec(session_id, command, &token)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        let token = self.require_token().ok()?;
        match self.api.exec_state(exec_id, &token) {
            Ok(state) => Some(state),
            Err(ContainerError::ExecNotFound) => None,
            Err(err) => Some(ExecState::Failed {
                error: err.to_string(),
                at: Timestamp::now(),
            }),
        }
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let token = self.require_token()?;
        let cursor = {
            let guard = self.exec_cursors.lock().unwrap_or_else(|e| e.into_inner());
            guard.get(exec_id).cloned()
        };
        let (chunk, next) = self
            .api
            .poll_exec_output(exec_id, cursor.as_deref(), &token)?;
        if let Some(next) = next {
            let mut guard = self.exec_cursors.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(exec_id.to_string(), next);
        }
        Ok(chunk)
    }

    fn cancel_exec(&self, _exec_id: &str) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "cancel_exec".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        let token = self.require_token()?;
        self.api.read_file(session_id, path, offset, len, &token)
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError> {
        let token = self.require_token()?;
        self.api.write_file(session_id, path, offset, data, &token)
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let token = self.require_token()?;
        self.api.stop(session_id, &token)
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let token = self.require_token()?;
        let cursor = {
            let guard = self
                .session_cursors
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard.get(session_id).cloned()
        };
        let (chunk, next) = self
            .api
            .poll_output(session_id, cursor.as_deref(), &token)?;
        if let Some(next) = next {
            let mut guard = self
                .session_cursors
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard.insert(session_id.to_string(), next);
        }
        Ok(chunk)
    }
}

#[cfg(not(target_arch = "wasm32"))]
const DAYTONA_DEFAULT_SNAPSHOT: &str = "daytonaio/sandbox:latest";
#[cfg(not(target_arch = "wasm32"))]
const OUTPUT_CHUNK_SIZE: usize = 64 * 1024;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct DaytonaProviderConfig {
    base_url: String,
    snapshot: Option<String>,
    target: Option<String>,
    organization_id: Option<String>,
    auto_stop_minutes: Option<i32>,
    auto_delete_minutes: Option<i32>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaProviderConfig {
    fn from_env() -> (Self, Option<String>) {
        let base_url = std::env::var(DAYTONA_API_URL_ENV)
            .or_else(|_| std::env::var(DAYTONA_BASE_URL_ENV))
            .unwrap_or_else(|_| "https://api.daytona.io".to_string());
        let api_key = std::env::var(DAYTONA_API_KEY_ENV).ok();
        let snapshot = std::env::var(DAYTONA_SNAPSHOT_ENV)
            .or_else(|_| std::env::var(DAYTONA_DEFAULT_SNAPSHOT_ENV))
            .ok();
        let target = std::env::var(DAYTONA_TARGET_ENV).ok();
        let organization_id = std::env::var(DAYTONA_ORG_ID_ENV).ok();
        let auto_stop_minutes = parse_env_i32(DAYTONA_AUTO_STOP_ENV);
        let auto_delete_minutes = parse_env_i32(DAYTONA_AUTO_DELETE_ENV);
        (
            Self {
                base_url,
                snapshot,
                target,
                organization_id,
                auto_stop_minutes,
                auto_delete_minutes,
            },
            api_key,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_env_i32(name: &str) -> Option<i32> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_daytona_snapshot(request: &ContainerRequest, config: &DaytonaProviderConfig) -> String {
    request
        .image
        .clone()
        .or_else(|| config.snapshot.clone())
        .unwrap_or_else(|| DAYTONA_DEFAULT_SNAPSHOT.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn wrap_shell_command(command: &str) -> String {
    let shell_metacharacters = ['|', '>', '<', '&', ';', '$', '`', '(', ')', '{', '}', '\n'];
    if command.contains(shell_metacharacters.as_ref()) {
        let escaped = command.replace('\'', "'\"'\"'");
        format!("sh -lc '{}'", escaped)
    } else {
        command.to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_daytona_snapshot_resource_error(err: &daytona::DaytonaError) -> bool {
    matches!(
        err,
        daytona::DaytonaError::ApiError { message, .. }
            if message.contains("Cannot specify Sandbox resources")
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn is_daytona_snapshot_not_found(err: &daytona::DaytonaError) -> bool {
    matches!(
        err,
        daytona::DaytonaError::ApiError { message, .. }
            if message.contains("Snapshot") && message.contains("not found")
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn join_daytona_path(base: &str, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn push_output_chunks(
    queue: &mut VecDeque<OutputChunk>,
    session_id: &str,
    exec_id: Option<&str>,
    stream: OutputStream,
    data: &str,
) {
    if data.is_empty() {
        return;
    }
    let exec_id = exec_id.map(|id| id.to_string());
    for chunk in data.as_bytes().chunks(OUTPUT_CHUNK_SIZE) {
        let text = String::from_utf8_lossy(chunk).to_string();
        queue.push_back(OutputChunk {
            session_id: session_id.to_string(),
            exec_id: exec_id.clone(),
            stream: stream.clone(),
            data: text,
        });
    }
}

