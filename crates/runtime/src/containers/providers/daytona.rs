/// Daytona SDK-backed container provider.
#[cfg(not(target_arch = "wasm32"))]
pub struct DaytonaContainerProvider {
    client: Arc<DaytonaClient>,
    config: DaytonaProviderConfig,
    executor: AsyncExecutor,
    sessions: Arc<RwLock<HashMap<String, Arc<DaytonaSession>>>>,
    execs: Arc<RwLock<HashMap<String, Arc<DaytonaExec>>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaContainerProvider {
    pub fn from_env() -> Result<Option<Self>, ContainerError> {
        let (config, api_key) = DaytonaProviderConfig::from_env();
        let Some(api_key) = api_key else {
            return Ok(None);
        };
        let mut daytona_config = DaytonaConfig::with_api_key(api_key).base_url(&config.base_url);
        if let Some(org_id) = config.organization_id.clone() {
            daytona_config = daytona_config.organization_id(org_id);
        }
        let client = DaytonaClient::new(daytona_config)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Self::new(client, config).map(Some)
    }

    fn new(client: DaytonaClient, config: DaytonaProviderConfig) -> Result<Self, ContainerError> {
        let executor = AsyncExecutor::new()?;
        Ok(Self {
            client: Arc::new(client),
            config,
            executor,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn session(&self, session_id: &str) -> Result<Arc<DaytonaSession>, ContainerError> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn exec(&self, exec_id: &str) -> Result<Arc<DaytonaExec>, ContainerError> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
            .ok_or(ContainerError::ExecNotFound)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ContainerProvider for DaytonaContainerProvider {
    fn id(&self) -> &str {
        "daytona"
    }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "daytona".to_string(),
            name: "Daytona Cloud Sandbox".to_string(),
            available_images: Vec::new(),
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: true,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 5000,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 20_480,
                max_time_secs: 3600,
                network_allowed: true,
            },
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        if request
            .repo
            .as_ref()
            .and_then(|repo| repo.auth.as_ref())
            .is_some()
        {
            return Err(ContainerError::NotSupported {
                capability: "repo_auth".to_string(),
                provider: "daytona".to_string(),
            });
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let session = Arc::new(DaytonaSession::new(session_id.clone(), request));
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), session.clone());

        let client = self.client.clone();
        let config = self.config.clone();
        self.executor.spawn(async move {
            if let Err(err) = run_daytona_session(client, session.clone(), config).await {
                session.fail(&err.to_string());
            }
        });

        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|session| {
                session
                    .state
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
            })
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        let exec_id = uuid::Uuid::new_v4().to_string();
        let exec_id_clone = exec_id.clone();
        let exec = Arc::new(DaytonaExec::new());
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(exec_id.clone(), exec.clone());

        let client = self.client.clone();
        let execs = self.execs.clone();
        let command_string = command.to_string();
        let session_clone = session.clone();
        self.executor.spawn(async move {
            exec.running();
            let start = Instant::now();
            let wrapped = wrap_shell_command(&command_string);
            let mut request = ExecuteRequest::new(wrapped)
                .timeout(session_clone.request.limits.max_time_secs as i32);
            if let Some(workdir) = session_clone.workdir() {
                request = request.cwd(workdir);
            }

            match client.execute_command(&sandbox_id, &request).await {
                Ok(response) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    let result = CommandResult {
                        command: command_string.clone(),
                        exit_code: response.exit_code(),
                        stdout: response.result.clone(),
                        stderr: String::new(),
                        duration_ms,
                    };
                    exec.complete(result.clone());
                    exec.push_output(
                        &session_clone.session_id,
                        Some(&exec_id_clone),
                        OutputStream::Stdout,
                        &response.result,
                    );
                    session_clone.push_output(
                        Some(&exec_id_clone),
                        OutputStream::Stdout,
                        &response.result,
                    );
                }
                Err(err) => {
                    let message = err.to_string();
                    exec.fail(&message);
                    exec.push_output(
                        &session_clone.session_id,
                        Some(&exec_id_clone),
                        OutputStream::Stderr,
                        &message,
                    );
                    session_clone.push_output(Some(&exec_id_clone), OutputStream::Stderr, &message);
                }
            }

            execs
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&exec_id_clone);
        });

        Ok(exec_id)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .map(|exec| exec.state.read().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let exec = self.exec(exec_id)?;
        Ok(exec.pop_output())
    }

    fn cancel_exec(&self, exec_id: &str) -> Result<(), ContainerError> {
        let exec = self.exec(exec_id)?;
        exec.fail("cancelled");
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(exec_id);
        Ok(())
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let full_path = session.resolve_path(path);
        let data = self
            .executor
            .block_on(self.client.download_file(&sandbox_id, &full_path))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let start = offset as usize;
        if start >= data.len() {
            return Ok(Vec::new());
        }
        let end = (offset + len).min(data.len() as u64) as usize;
        Ok(data[start..end].to_vec())
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        let full_path = session.resolve_path(path);
        let payload = if offset == 0 {
            data.to_vec()
        } else {
            let mut existing = self
                .executor
                .block_on(self.client.download_file(&sandbox_id, &full_path))
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            let start = offset as usize;
            let end = start.saturating_add(data.len());
            if existing.len() < start {
                existing.resize(start, 0);
            }
            if existing.len() < end {
                existing.resize(end, 0);
            }
            existing[start..end].copy_from_slice(data);
            existing
        };
        self.executor
            .block_on(self.client.upload_file(&sandbox_id, &full_path, &payload))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let sandbox_id = session.sandbox_id()?;
        let _ = self
            .executor
            .block_on(self.client.stop_sandbox(&sandbox_id));
        let _ = self
            .executor
            .block_on(self.client.delete_sandbox(&sandbox_id, false));
        session.expire();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session_id);
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let session = self.session(session_id)?;
        Ok(session.pop_output())
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct DaytonaSession {
    session_id: String,
    request: ContainerRequest,
    state: RwLock<SessionState>,
    output: Mutex<VecDeque<OutputChunk>>,
    sandbox_id: Mutex<Option<String>>,
    workdir: Mutex<Option<String>>,
    start: Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaSession {
    fn new(session_id: String, request: ContainerRequest) -> Self {
        Self {
            session_id: session_id.clone(),
            state: RwLock::new(SessionState::Provisioning {
                started_at: Timestamp::now(),
            }),
            output: Mutex::new(VecDeque::new()),
            request,
            sandbox_id: Mutex::new(None),
            workdir: Mutex::new(None),
            start: Instant::now(),
        }
    }

    fn sandbox_id(&self) -> Result<String, ContainerError> {
        self.sandbox_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(ContainerError::NotReady)
    }

    fn set_sandbox_id(&self, id: String) {
        *self.sandbox_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    }

    fn set_workdir(&self, workdir: Option<String>) {
        *self.workdir.lock().unwrap_or_else(|e| e.into_inner()) = workdir;
    }

    fn workdir(&self) -> Option<String> {
        self.workdir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn resolve_path(&self, path: &str) -> String {
        match self.workdir() {
            Some(base) => join_daytona_path(&base, path),
            None => path.to_string(),
        }
    }

    fn push_output(&self, exec_id: Option<&str>, stream: OutputStream, data: &str) {
        let mut guard = self.output.lock().unwrap_or_else(|e| e.into_inner());
        push_output_chunks(&mut guard, &self.session_id, exec_id, stream, data);
    }

    fn pop_output(&self) -> Option<OutputChunk> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
    }

    fn set_state(&self, state: SessionState) {
        let mut guard = self.state.write().unwrap_or_else(|e| e.into_inner());
        *guard = state;
    }

    fn fail(&self, message: &str) {
        self.set_state(SessionState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        });
    }

    fn expire(&self) {
        self.set_state(SessionState::Expired {
            at: Timestamp::now(),
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct DaytonaExec {
    state: RwLock<ExecState>,
    output: Mutex<VecDeque<OutputChunk>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DaytonaExec {
    fn new() -> Self {
        Self {
            state: RwLock::new(ExecState::Pending {
                submitted_at: Timestamp::now(),
            }),
            output: Mutex::new(VecDeque::new()),
        }
    }

    fn running(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Running {
            started_at: Timestamp::now(),
        };
    }

    fn complete(&self, result: CommandResult) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Complete(result);
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }

    fn push_output(
        &self,
        session_id: &str,
        exec_id: Option<&str>,
        stream: OutputStream,
        data: &str,
    ) {
        let mut guard = self.output.lock().unwrap_or_else(|e| e.into_inner());
        push_output_chunks(&mut guard, session_id, exec_id, stream, data);
    }

    fn pop_output(&self) -> Option<OutputChunk> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_daytona_session(
    client: Arc<DaytonaClient>,
    session: Arc<DaytonaSession>,
    config: DaytonaProviderConfig,
) -> Result<(), ContainerError> {
    let request = session.request.clone();
    let snapshot = resolve_daytona_snapshot(&request, &config);

    let mut create = CreateSandbox::new(snapshot.clone());
    if let Some(target) = config.target.as_ref() {
        create = create.target(target.clone());
    }
    if !request.env.is_empty() {
        create = create.env(request.env.clone());
    }
    if let Some(minutes) = config.auto_stop_minutes {
        create = create.auto_stop_interval(minutes);
    }
    if let Some(minutes) = config.auto_delete_minutes {
        create = create.auto_delete_interval(minutes);
    }

    let mut labels = HashMap::new();
    labels.insert(
        "openagents_session_id".to_string(),
        session.session_id.clone(),
    );
    create = create.labels(labels);

    let base_create = create.clone();
    let cpu = request.limits.max_cpu_cores.ceil().max(1.0) as i32;
    let memory_gb = ((request.limits.max_memory_mb as f64) / 1024.0).ceil() as i32;
    let disk_gb = ((request.limits.max_disk_mb as f64) / 1024.0).ceil() as i32;
    if cpu > 0 {
        create = create.cpu(cpu);
    }
    if memory_gb > 0 {
        create = create.memory(memory_gb);
    }
    if disk_gb > 0 {
        create = create.disk(disk_gb);
    }

    let sandbox = match client.create_sandbox(&create).await {
        Ok(sandbox) => sandbox,
        Err(err) if is_daytona_snapshot_resource_error(&err) => {
            match client.create_sandbox(&base_create).await {
                Ok(sandbox) => sandbox,
                Err(err) if is_daytona_snapshot_not_found(&err) => {
                    return Err(ContainerError::ProviderError(
                        "Daytona snapshot not found. Set DAYTONA_SNAPSHOT to a snapshot you can access."
                            .to_string(),
                    ));
                }
                Err(err) => return Err(ContainerError::ProviderError(err.to_string())),
            }
        }
        Err(err) if is_daytona_snapshot_not_found(&err) => {
            return Err(ContainerError::ProviderError(
                "Daytona snapshot not found. Set DAYTONA_SNAPSHOT to a snapshot you can access."
                    .to_string(),
            ));
        }
        Err(err) => return Err(ContainerError::ProviderError(err.to_string())),
    };
    session.set_sandbox_id(sandbox.id.clone());

    client
        .start_sandbox(&sandbox.id)
        .await
        .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

    let timeout_ms = request.timeout_ms.unwrap_or_else(default_timeout_ms);
    client
        .wait_for_state(
            &sandbox.id,
            DaytonaSandboxState::Started,
            Duration::from_millis(timeout_ms),
        )
        .await
        .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

    session.set_state(SessionState::Running {
        started_at: Timestamp::now(),
        commands_completed: 0,
    });

    let mut workdir = request.workdir.clone();
    if let Some(repo) = request.repo.clone() {
        session.set_state(SessionState::Cloning {
            started_at: Timestamp::now(),
            repo_url: repo.url.clone(),
        });

        let project_dir = client
            .get_project_dir(&sandbox.id)
            .await
            .unwrap_or_else(|_| "/workspace".to_string());
        let mut clone_request = GitCloneRequest::new(repo.url.clone(), project_dir.clone());
        if repo.git_ref.len() >= 40 && repo.git_ref.chars().all(|c| c.is_ascii_hexdigit()) {
            clone_request = clone_request.commit_id(repo.git_ref.clone());
        } else {
            clone_request = clone_request.branch(repo.git_ref.clone());
        }
        client
            .git_clone(&sandbox.id, &clone_request)
            .await
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

        let suffix = join_workdir(&repo.subdir, &request.workdir);
        workdir = Some(match suffix {
            Some(path) => join_daytona_path(&project_dir, &path),
            None => project_dir,
        });

        session.set_state(SessionState::Running {
            started_at: Timestamp::now(),
            commands_completed: 0,
        });
    } else if workdir.is_none() {
        if let Ok(project_dir) = client.get_project_dir(&sandbox.id).await {
            workdir = Some(project_dir);
        }
    }

    session.set_workdir(workdir.clone());

    let mut command_results = Vec::new();
    let mut combined_exit = 0;
    for (idx, command) in request.commands.iter().enumerate() {
        let start = Instant::now();
        let wrapped = wrap_shell_command(command);
        let mut exec_request =
            ExecuteRequest::new(wrapped).timeout(request.limits.max_time_secs as i32);
        if let Some(dir) = workdir.clone() {
            exec_request = exec_request.cwd(dir);
        }

        let response = client
            .execute_command(&sandbox.id, &exec_request)
            .await
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let result = CommandResult {
            command: command.clone(),
            exit_code: response.exit_code(),
            stdout: response.result.clone(),
            stderr: String::new(),
            duration_ms,
        };
        session.push_output(None, OutputStream::Stdout, &response.result);
        command_results.push(result.clone());

        if response.exit_code() != 0 {
            session.fail("command failed");
            return Ok(());
        }

        combined_exit = response.exit_code();
        session.set_state(SessionState::Running {
            started_at: Timestamp::now(),
            commands_completed: idx + 1,
        });
    }

    if matches!(request.kind, ContainerKind::Interactive) {
        return Ok(());
    }

    let stdout = command_results
        .iter()
        .map(|r| r.stdout.clone())
        .collect::<Vec<_>>()
        .join("");
    let stderr = command_results
        .iter()
        .map(|r| r.stderr.clone())
        .collect::<Vec<_>>()
        .join("");
    let response = ContainerResponse {
        session_id: session.session_id.clone(),
        exit_code: Some(combined_exit),
        stdout,
        stderr,
        command_results,
        artifacts: Vec::new(),
        usage: ContainerUsage::zero(),
        cost_usd: request.max_cost_usd.unwrap_or(0),
        reserved_usd: request.max_cost_usd.unwrap_or(0),
        duration_ms: session.start.elapsed().as_millis() as u64,
        provider_id: "daytona".to_string(),
    };
    session.set_state(SessionState::Complete(response));
    Ok(())
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[derive(Default)]
struct RemoteSessionState {
    remote_id: Option<String>,
    cursor: Option<String>,
    queue: VecDeque<OutputChunk>,
    refreshing: bool,
    streaming: bool,
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[derive(Default)]
struct RemoteExecState {
    remote_id: Option<String>,
    cursor: Option<String>,
    queue: VecDeque<OutputChunk>,
    session_id: String,
    refreshing: bool,
    streaming: bool,
}

