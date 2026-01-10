/// Local container provider using the Docker CLI.
#[cfg(not(target_arch = "wasm32"))]
pub struct LocalContainerProvider {
    sessions: Arc<RwLock<HashMap<String, Arc<LocalSession>>>>,
    execs: Arc<RwLock<HashMap<String, Arc<LocalExec>>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalContainerProvider {
    /// Create a new local provider.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn docker_available() -> bool {
        Command::new("docker")
            .arg("version")
            .arg("--format")
            .arg("{{.Server.Version}}")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    fn docker_images() -> Result<Vec<String>, ContainerError> {
        let output = Command::new("docker")
            .args(["images", "--format", "{{.Repository}}:{{.Tag}}"])
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if !output.status.success() {
            return Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let images = text
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .filter(|line| !line.contains("<none>"))
            .collect();
        Ok(images)
    }

    fn ensure_available() -> Result<(), ContainerError> {
        if Self::docker_available() {
            Ok(())
        } else {
            Err(ContainerError::Unavailable(
                "docker not available".to_string(),
            ))
        }
    }

    fn session(&self, session_id: &str) -> Result<Arc<LocalSession>, ContainerError> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn exec(&self, exec_id: &str) -> Result<Arc<LocalExec>, ContainerError> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
            .ok_or(ContainerError::ExecNotFound)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for LocalContainerProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ContainerProvider for LocalContainerProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn info(&self) -> ContainerProviderInfo {
        let available = Self::docker_available();
        let images = if available {
            Self::docker_images().unwrap_or_default()
        } else {
            Vec::new()
        };
        ContainerProviderInfo {
            id: "local".to_string(),
            name: "Local (Docker)".to_string(),
            available_images: images,
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: true,
                interactive: true,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 0,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 16384,
                max_cpu_cores: 8.0,
                max_disk_mb: 10240,
                max_time_secs: 3600,
                network_allowed: true,
            },
            status: if available {
                ProviderStatus::Available
            } else {
                ProviderStatus::Unavailable {
                    reason: "docker not available".to_string(),
                }
            },
        }
    }

    fn is_available(&self) -> bool {
        Self::docker_available()
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        Self::ensure_available()?;
        let image = request
            .image
            .clone()
            .ok_or_else(|| ContainerError::InvalidRequest("image required".to_string()))?;

        if request
            .repo
            .as_ref()
            .and_then(|r| r.auth.as_ref())
            .is_some()
        {
            return Err(ContainerError::NotSupported {
                capability: "repo_auth".to_string(),
                provider: "local".to_string(),
            });
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let session_id_clone = session_id.clone();
        let (output_tx, output_rx) = mpsc::channel(256);
        let session = Arc::new(LocalSession::new(
            session_id.clone(),
            request.clone(),
            output_tx,
            output_rx,
        ));

        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), session.clone());

        let provider_sessions = self.sessions.clone();
        thread::spawn(move || {
            if let Err(err) = session.run(image) {
                session.fail(&err.to_string());
                provider_sessions
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&session_id_clone);
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
        let container_id = session.container_id()?;
        let exec_id = uuid::Uuid::new_v4().to_string();
        let exec_id_clone = exec_id.clone();
        let (output_tx, output_rx) = mpsc::channel(128);
        let exec = Arc::new(LocalExec::new(
            exec_id.clone(),
            session_id.to_string(),
            output_tx,
            output_rx,
        ));
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(exec_id.clone(), exec.clone());

        let provider_execs = self.execs.clone();
        let session_clone = session.clone();
        let command_string = command.to_string();
        thread::spawn(move || {
            exec.running();
            let result = run_exec_command(
                &container_id,
                &session_clone.session_id,
                &command_string,
                session_clone.request.workdir.clone(),
                session_clone.request.env.clone(),
                session_clone.request.limits.max_time_secs,
                &exec.output_tx,
                Some(exec_id_clone.clone()),
                &session_clone.output_tx,
            );
            match result {
                Ok(command_result) => exec.complete(command_result),
                Err(err) => exec.fail(&err.to_string()),
            }
            provider_execs
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
        let mut rx = exec.output_rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn cancel_exec(&self, exec_id: &str) -> Result<(), ContainerError> {
        let exec = self.exec(exec_id)?;
        exec.fail("cancelled");
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
        let container_id = session.container_id()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let escaped = shell_escape(path);
        let script = format!(
            "dd if={} bs=1 skip={} count={} 2>/dev/null",
            escaped, offset, len
        );
        let output = Command::new("docker")
            .args(["exec", &container_id, "sh", "-lc", &script])
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let escaped = shell_escape(path);
        if offset == 0 {
            let truncate = format!("truncate -s 0 {} 2>/dev/null || : ", escaped);
            let _ = Command::new("docker")
                .args(["exec", &container_id, "sh", "-lc", &truncate])
                .output();
        }
        let script = format!(
            "dd of={} bs=1 seek={} conv=notrunc 2>/dev/null",
            escaped, offset
        );
        let mut child = Command::new("docker")
            .args(["exec", "-i", &container_id, "sh", "-lc", &script])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(data)
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let session = self.session(session_id)?;
        let container_id = session.container_id()?;
        let _ = Command::new("docker")
            .args(["stop", &container_id])
            .output();
        let _ = Command::new("docker").args(["rm", &container_id]).output();
        session.expire();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session_id);
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let session = self.session(session_id)?;
        let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct LocalSession {
    session_id: String,
    state: RwLock<SessionState>,
    output_tx: mpsc::Sender<OutputChunk>,
    output_rx: Mutex<mpsc::Receiver<OutputChunk>>,
    request: ContainerRequest,
    container_id: Mutex<Option<String>>,
    start: Instant,
    repo_dir: Mutex<Option<tempfile::TempDir>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalSession {
    fn new(
        session_id: String,
        request: ContainerRequest,
        output_tx: mpsc::Sender<OutputChunk>,
        output_rx: mpsc::Receiver<OutputChunk>,
    ) -> Self {
        Self {
            session_id: session_id.clone(),
            state: RwLock::new(SessionState::Provisioning {
                started_at: Timestamp::now(),
            }),
            output_tx,
            output_rx: Mutex::new(output_rx),
            request,
            container_id: Mutex::new(None),
            start: Instant::now(),
            repo_dir: Mutex::new(None),
        }
    }

    fn container_id(&self) -> Result<String, ContainerError> {
        self.container_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(ContainerError::SessionNotFound)
    }

    fn set_container_id(&self, id: String) {
        *self.container_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    }

    fn run(&self, image: String) -> Result<(), ContainerError> {
        let mut workdir = self.request.workdir.clone();
        if let Some(repo) = self.request.repo.clone() {
            let repo_dir = tempfile::TempDir::new()
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            {
                let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
                *state = SessionState::Cloning {
                    started_at: Timestamp::now(),
                    repo_url: repo.url.clone(),
                };
            }
            let status = Command::new("git")
                .args([
                    "clone",
                    "--depth",
                    "1",
                    "--branch",
                    &repo.git_ref,
                    &repo.url,
                    repo_dir.path().to_str().unwrap_or_default(),
                ])
                .status()
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            if !status.success() {
                return Err(ContainerError::ProviderError(
                    "git clone failed".to_string(),
                ));
            }
            let mut repo_guard = self.repo_dir.lock().unwrap_or_else(|e| e.into_inner());
            *repo_guard = Some(repo_dir);
            let base = "/workspace".to_string();
            workdir = Some(match repo.subdir {
                Some(subdir) => format!("{}/{}", base, subdir.trim_matches('/')),
                None => base,
            });
        }

        let mut cmd = Command::new("docker");
        cmd.arg("run").arg("-d");
        cmd.args(["--name", &self.session_id]);
        cmd.arg(format!("--memory={}m", self.request.limits.max_memory_mb));
        cmd.arg(format!("--cpus={}", self.request.limits.max_cpu_cores));
        if !self.request.limits.allow_network {
            cmd.args(["--network", "none"]);
        }
        for (key, value) in &self.request.env {
            cmd.arg("-e").arg(format!("{}={}", key, value));
        }
        if let Some(ref dir) = workdir {
            cmd.args(["-w", dir]);
        }
        if let Some(repo_dir) = self
            .repo_dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            let mount = format!("{}:/workspace", repo_dir.path().display());
            cmd.arg("-v").arg(mount);
        }
        cmd.arg(&image);
        cmd.args(["sh", "-lc", "sleep infinity"]);
        let output = cmd
            .output()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if !output.status.success() {
            return Err(ContainerError::ProviderError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        self.set_container_id(container_id.clone());

        {
            let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
            *state = SessionState::Running {
                started_at: Timestamp::now(),
                commands_completed: 0,
            };
        }

        let mut command_results = Vec::new();
        let mut combined_exit = 0;
        for (idx, command) in self.request.commands.iter().enumerate() {
            let result = run_exec_command(
                &container_id,
                &self.session_id,
                command,
                workdir.clone(),
                self.request.env.clone(),
                self.request.limits.max_time_secs,
                &self.output_tx,
                None,
                &self.output_tx,
            )?;
            if result.exit_code != 0 {
                command_results.push(result);
                let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
                *state = SessionState::Failed {
                    error: "command failed".to_string(),
                    at: Timestamp::now(),
                };
                return Ok(());
            }
            let exit_code = result.exit_code;
            command_results.push(result);
            combined_exit = exit_code;
            let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
            *state = SessionState::Running {
                started_at: Timestamp::now(),
                commands_completed: idx + 1,
            };
        }

        if matches!(self.request.kind, ContainerKind::Interactive) {
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
            session_id: self.session_id.clone(),
            exit_code: Some(combined_exit),
            stdout,
            stderr,
            command_results,
            artifacts: Vec::new(),
            usage: ContainerUsage::zero(),
            cost_usd: 0,
            reserved_usd: self.request.max_cost_usd.unwrap_or(0),
            duration_ms: self.start.elapsed().as_millis() as u64,
            provider_id: "local".to_string(),
        };
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Complete(response);
        Ok(())
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }

    fn expire(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = SessionState::Expired {
            at: Timestamp::now(),
        };
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct LocalExec {
    state: RwLock<ExecState>,
    output_tx: mpsc::Sender<OutputChunk>,
    output_rx: Mutex<mpsc::Receiver<OutputChunk>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalExec {
    fn new(
        _exec_id: String,
        _session_id: String,
        output_tx: mpsc::Sender<OutputChunk>,
        output_rx: mpsc::Receiver<OutputChunk>,
    ) -> Self {
        Self {
            state: RwLock::new(ExecState::Pending {
                submitted_at: Timestamp::now(),
            }),
            output_tx,
            output_rx: Mutex::new(output_rx),
        }
    }

    fn complete(&self, result: CommandResult) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Complete(result);
    }

    fn running(&self) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Running {
            started_at: Timestamp::now(),
        };
    }

    fn fail(&self, message: &str) {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        *state = ExecState::Failed {
            error: message.to_string(),
            at: Timestamp::now(),
        };
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_exec_command(
    container_id: &str,
    session_id: &str,
    command: &str,
    workdir: Option<String>,
    env: HashMap<String, String>,
    max_time_secs: u32,
    exec_tx: &mpsc::Sender<OutputChunk>,
    exec_id: Option<String>,
    session_tx: &mpsc::Sender<OutputChunk>,
) -> Result<CommandResult, ContainerError> {
    let mut cmd = Command::new("docker");
    cmd.arg("exec").arg("-i");
    if let Some(ref dir) = workdir {
        cmd.args(["-w", dir]);
    }
    for (key, value) in env {
        cmd.arg("-e").arg(format!("{}={}", key, value));
    }
    cmd.arg(container_id);
    cmd.args(["sh", "-lc", command]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ContainerError::ProviderError("missing stdout pipe".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ContainerError::ProviderError("missing stderr pipe".to_string()))?;

    let exec_id_clone = exec_id.clone();
    let session_id = session_id.to_string();
    let out_tx = exec_tx.clone();
    let session_tx_clone = session_tx.clone();
    let stdout_handle = spawn_reader(
        stdout,
        OutputStream::Stdout,
        session_id.clone(),
        exec_id_clone.clone(),
        out_tx,
        session_tx_clone,
    );
    let err_tx = exec_tx.clone();
    let session_tx_clone = session_tx.clone();
    let stderr_handle = spawn_reader(
        stderr,
        OutputStream::Stderr,
        session_id.clone(),
        exec_id_clone,
        err_tx,
        session_tx_clone,
    );

    let start = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?
        {
            let stdout_text = stdout_handle.join().unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle.join().unwrap_or_else(|_| String::new());
            let exit_code = status.code().unwrap_or(-1);
            return Ok(CommandResult {
                command: command.to_string(),
                exit_code,
                stdout: stdout_text,
                stderr: stderr_text,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
        if start.elapsed().as_secs() > max_time_secs as u64 {
            let _ = child.kill();
            let stdout_text = stdout_handle.join().unwrap_or_else(|_| String::new());
            let stderr_text = stderr_handle.join().unwrap_or_else(|_| String::new());
            return Err(ContainerError::ProviderError(format!(
                "command timeout after {}s (stdout={} stderr={})",
                max_time_secs, stdout_text, stderr_text
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
    stream: OutputStream,
    session_id: String,
    exec_id: Option<String>,
    exec_tx: mpsc::Sender<OutputChunk>,
    session_tx: mpsc::Sender<OutputChunk>,
) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut output = String::new();
        loop {
            let read = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let chunk = String::from_utf8_lossy(&buf[..read]).to_string();
            output.push_str(&chunk);
            let payload = OutputChunk {
                session_id: session_id.clone(),
                exec_id: exec_id.clone(),
                stream: stream.clone(),
                data: chunk,
            };
            let _ = exec_tx.send(payload.clone());
            let _ = session_tx.send(payload);
        }
        output
    })
}

fn shell_escape(value: &str) -> String {
    let mut escaped = String::new();
    escaped.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            escaped.push_str("'\\''");
        } else {
            escaped.push(ch);
        }
    }
    escaped.push('\'');
    escaped
}
