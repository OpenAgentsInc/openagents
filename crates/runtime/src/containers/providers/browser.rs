/// OpenAgents API-backed container provider for browser targets.
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub struct WasmOpenAgentsContainerProvider {
    provider_id: String,
    name: String,
    base_url: String,
    auth: Arc<OpenAgentsAuth>,
    info: Arc<RwLock<ContainerProviderInfo>>,
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
    remote_sessions: Arc<Mutex<HashMap<String, RemoteSessionState>>>,
    remote_execs: Arc<Mutex<HashMap<String, RemoteExecState>>>,
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl WasmOpenAgentsContainerProvider {
    /// Create a new OpenAgents API-backed provider (browser).
    pub fn new(
        provider_id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        let provider_id = provider_id.into();
        let name = name.into();
        let info = ContainerProviderInfo {
            id: provider_id.clone(),
            name: name.clone(),
            available_images: Vec::new(),
            capabilities: ContainerCapabilities {
                git_clone: true,
                file_access: false,
                interactive: false,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: ContainerLatency {
                startup_ms: 0,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 10240,
                max_time_secs: 3600,
                network_allowed: true,
            },
            status: ProviderStatus::Degraded {
                reason: "loading provider info".to_string(),
            },
        };
        let provider = Self {
            provider_id,
            name,
            base_url: base_url.into(),
            auth,
            info: Arc::new(RwLock::new(info)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
            remote_sessions: Arc::new(Mutex::new(HashMap::new())),
            remote_execs: Arc::new(Mutex::new(HashMap::new())),
        };
        provider.spawn_info_refresh();
        provider
    }

    pub fn cloudflare(base_url: impl Into<String>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("cloudflare", "Cloudflare Containers", base_url, auth)
    }

    pub fn daytona(base_url: impl Into<String>, auth: Arc<OpenAgentsAuth>) -> Self {
        Self::new("daytona", "Daytona Cloud Sandbox", base_url, auth)
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn require_token(&self) -> Result<String, ContainerError> {
        self.auth.token().ok_or(ContainerError::AuthRequired {
            provider: self.provider_id.clone(),
            message: "OpenAgents API token required".to_string(),
        })
    }

    fn spawn_info_refresh(&self) {
        let info = Arc::clone(&self.info);
        let provider_id = self.provider_id.clone();
        let name = self.name.clone();
        let url = self.url(&format!("containers/providers/{}/info", provider_id));
        let auth = Arc::clone(&self.auth);
        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let updated = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<ContainerProviderInfo>(&bytes).unwrap_or_else(|err| {
                        unavailable_provider_info(
                            &provider_id,
                            &name,
                            format!("invalid provider info: {}", err),
                        )
                    })
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    unavailable_provider_info(
                        &provider_id,
                        &name,
                        format!("openagents api {}: {}", status, body),
                    )
                }
                Err(err) => unavailable_provider_info(&provider_id, &name, err),
            };
            let mut guard = info.write().unwrap_or_else(|e| e.into_inner());
            *guard = updated;
        });
    }

    fn spawn_session_refresh(&self, session_id: &str) {
        let (remote_id, url, auth, sessions, remote_sessions, session_id) = {
            let mut guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(session_id) {
                Some(state) => state,
                None => return,
            };
            if state.refreshing {
                return;
            }
            let remote_id = match state.remote_id.clone() {
                Some(id) => id,
                None => return,
            };
            state.refreshing = true;
            let url = self.url(&format!("containers/sessions/{}", remote_id));
            (
                remote_id,
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.sessions),
                Arc::clone(&self.remote_sessions),
                session_id.to_string(),
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let next_state = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<SessionState>(&bytes)
                        .map_err(|err| format!("invalid session state: {}", err))
                }
                Ok((404, _)) => Err("session not found".to_string()),
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    Err(format!("openagents api {}: {}", status, body))
                }
                Err(err) => Err(err),
            };

            match next_state {
                Ok(state) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(session_id.clone(), state);
                }
                Err(err) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id.clone(),
                        SessionState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }

            let mut guard = remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&session_id) {
                state.refreshing = false;
                state.remote_id = Some(remote_id);
            }
        });
    }

    fn spawn_session_output_poll(&self, session_id: &str) {
        let (url, auth, sessions, remote_sessions, session_id, cursor) = {
            let mut guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(session_id) {
                Some(state) => state,
                None => return,
            };
            if state.streaming {
                return;
            }
            let remote_id = match state.remote_id.clone() {
                Some(id) => id,
                None => return,
            };
            state.streaming = true;
            let cursor = state.cursor.clone();
            let url = match cursor.as_ref() {
                Some(cursor) => self.url(&format!(
                    "containers/sessions/{}/output?cursor={}",
                    remote_id, cursor
                )),
                None => self.url(&format!("containers/sessions/{}/output", remote_id)),
            };
            (
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.sessions),
                Arc::clone(&self.remote_sessions),
                session_id.to_string(),
                cursor,
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let mut next_chunk: Option<OutputChunk> = None;
            let mut next_cursor = cursor.clone();
            let mut error: Option<String> = None;

            match response {
                Ok((status, bytes)) if status == 204 || bytes.is_empty() => {}
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    #[derive(Deserialize)]
                    struct OutputResponse {
                        chunk: Option<OutputChunk>,
                        cursor: Option<String>,
                    }
                    if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
                        next_chunk = payload.chunk;
                        next_cursor = payload.cursor.or(next_cursor);
                    } else if let Ok(chunk) = serde_json::from_slice::<OutputChunk>(&bytes) {
                        next_chunk = Some(chunk);
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    error = Some(format!("openagents api {}: {}", status, body));
                }
                Err(err) => error = Some(err),
            }

            if let Some(err) = error {
                let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                guard.insert(
                    session_id.clone(),
                    SessionState::Failed {
                        error: err,
                        at: Timestamp::now(),
                    },
                );
            } else if let Some(chunk) = next_chunk {
                {
                    let mut guard = remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = guard.get_mut(&session_id) {
                        state.queue.push_back(chunk);
                        state.cursor = next_cursor.clone();
                    }
                }
            }

            let mut guard = remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&session_id) {
                state.streaming = false;
                state.cursor = next_cursor;
            }
        });
    }

    fn spawn_exec_refresh(&self, exec_id: &str) {
        let (remote_id, url, auth, execs, remote_execs, exec_id) = {
            let mut guard = self.remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(exec_id) {
                Some(state) => state,
                None => return,
            };
            if state.refreshing {
                return;
            }
            let remote_id = match state.remote_id.clone() {
                Some(id) => id,
                None => return,
            };
            state.refreshing = true;
            let url = self.url(&format!("containers/exec/{}", remote_id));
            (
                remote_id,
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.execs),
                Arc::clone(&self.remote_execs),
                exec_id.to_string(),
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let next_state = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<ExecState>(&bytes)
                        .map_err(|err| format!("invalid exec state: {}", err))
                }
                Ok((404, _)) => Err("exec not found".to_string()),
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    Err(format!("openagents api {}: {}", status, body))
                }
                Err(err) => Err(err),
            };

            match next_state {
                Ok(state) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(exec_id.clone(), state);
                }
                Err(err) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id.clone(),
                        ExecState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }

            let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&exec_id) {
                state.refreshing = false;
                state.remote_id = Some(remote_id);
            }
        });
    }

    fn spawn_exec_output_poll(&self, exec_id: &str) {
        let (url, auth, remote_execs, exec_id, cursor) = {
            let mut guard = self.remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(exec_id) {
                Some(state) => state,
                None => return,
            };
            if state.streaming {
                return;
            }
            let remote_id = match state.remote_id.clone() {
                Some(id) => id,
                None => return,
            };
            state.streaming = true;
            let cursor = state.cursor.clone();
            let url = match cursor.as_ref() {
                Some(cursor) => self.url(&format!(
                    "containers/exec/{}/output?cursor={}",
                    remote_id, cursor
                )),
                None => self.url(&format!("containers/exec/{}/output", remote_id)),
            };
            (
                url,
                Arc::clone(&self.auth),
                Arc::clone(&self.remote_execs),
                exec_id.to_string(),
                cursor,
            )
        };

        spawn_local(async move {
            let token = auth.token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let mut next_chunk: Option<OutputChunk> = None;
            let mut next_cursor = cursor.clone();
            let mut error: Option<String> = None;

            match response {
                Ok((status, bytes)) if status == 204 || bytes.is_empty() => {}
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    #[derive(Deserialize)]
                    struct OutputResponse {
                        chunk: Option<OutputChunk>,
                        cursor: Option<String>,
                    }
                    if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
                        next_chunk = payload.chunk;
                        next_cursor = payload.cursor.or(next_cursor);
                    } else if let Ok(chunk) = serde_json::from_slice::<OutputChunk>(&bytes) {
                        next_chunk = Some(chunk);
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    error = Some(format!("openagents api {}: {}", status, body));
                }
                Err(err) => error = Some(err),
            }

            if let Some(chunk) = next_chunk {
                let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(state) = guard.get_mut(&exec_id) {
                    state.queue.push_back(chunk);
                    state.cursor = next_cursor.clone();
                }
            }

            if error.is_some() {
                let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(state) = guard.get_mut(&exec_id) {
                    state.queue.push_back(OutputChunk {
                        session_id: exec_id.clone(),
                        exec_id: Some(exec_id.clone()),
                        stream: OutputStream::Stderr,
                        data: error.unwrap_or_else(|| "exec output error".to_string()),
                    });
                }
            }

            let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&exec_id) {
                state.streaming = false;
                state.cursor = next_cursor;
            }
        });
    }
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl ContainerProvider for WasmOpenAgentsContainerProvider {
    fn id(&self) -> &str {
        &self.provider_id
    }

    fn info(&self) -> ContainerProviderInfo {
        self.info.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn is_available(&self) -> bool {
        let info = self.info.read().unwrap_or_else(|e| e.into_inner());
        matches!(
            info.status,
            ProviderStatus::Available | ProviderStatus::Degraded { .. }
        )
    }

    fn requires_openagents_auth(&self) -> bool {
        true
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let started_at = Timestamp::now();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                SessionState::Provisioning { started_at },
            );
        self.remote_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), RemoteSessionState::default());

        let url = self.url(&format!(
            "containers/providers/{}/sessions",
            self.provider_id
        ));
        let sessions = Arc::clone(&self.sessions);
        let remote_sessions = Arc::clone(&self.remote_sessions);
        let session_id_clone = session_id.clone();

        spawn_local(async move {
            let body = match serde_json::to_string(&request) {
                Ok(body) => body,
                Err(err) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id_clone.clone(),
                        SessionState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let response = wasm_http::request_bytes("POST", &url, Some(&token), Some(body)).await;
            #[derive(Deserialize)]
            struct SessionResponse {
                session_id: String,
            }
            match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<SessionResponse>(&bytes) {
                        Ok(payload) => {
                            let mut guard =
                                remote_sessions.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(state) = guard.get_mut(&session_id_clone) {
                                state.remote_id = Some(payload.session_id);
                            }
                        }
                        Err(err) => {
                            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                session_id_clone.clone(),
                                SessionState::Failed {
                                    error: format!("invalid response: {}", err),
                                    at: Timestamp::now(),
                                },
                            );
                        }
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id_clone.clone(),
                        SessionState::Failed {
                            error: format!("openagents api {}: {}", status, body),
                            at: Timestamp::now(),
                        },
                    );
                }
                Err(err) => {
                    let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        session_id_clone.clone(),
                        SessionState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }
        });

        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let state = self
            .sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned();
        if let Some(state) = state.as_ref() {
            if !matches!(
                state,
                SessionState::Complete(_)
                    | SessionState::Failed { .. }
                    | SessionState::Expired { .. }
            ) {
                self.spawn_session_refresh(session_id);
            }
        }
        state
    }

    fn submit_exec(&self, session_id: &str, command: &str) -> Result<String, ContainerError> {
        let token = self.require_token()?;
        let remote_id = {
            let guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard
                .get(session_id)
                .and_then(|state| state.remote_id.clone())
        }
        .ok_or_else(|| ContainerError::InvalidRequest("session not ready".to_string()))?;

        let exec_id = uuid::Uuid::new_v4().to_string();
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                ExecState::Pending {
                    submitted_at: Timestamp::now(),
                },
            );
        self.remote_execs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                RemoteExecState {
                    session_id: session_id.to_string(),
                    ..RemoteExecState::default()
                },
            );

        let url = self.url(&format!("containers/sessions/{}/exec", remote_id));
        let execs = Arc::clone(&self.execs);
        let remote_execs = Arc::clone(&self.remote_execs);
        let exec_id_clone = exec_id.clone();
        let command = command.to_string();

        spawn_local(async move {
            let body = serde_json::json!({ "command": command });
            let body = match serde_json::to_string(&body) {
                Ok(body) => body,
                Err(err) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id_clone.clone(),
                        ExecState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let response = wasm_http::request_bytes("POST", &url, Some(&token), Some(body)).await;
            #[derive(Deserialize)]
            struct ExecResponse {
                exec_id: String,
            }
            match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<ExecResponse>(&bytes) {
                        Ok(payload) => {
                            let mut guard = remote_execs.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(state) = guard.get_mut(&exec_id_clone) {
                                state.remote_id = Some(payload.exec_id);
                            }
                            let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                exec_id_clone.clone(),
                                ExecState::Running {
                                    started_at: Timestamp::now(),
                                },
                            );
                        }
                        Err(err) => {
                            let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                exec_id_clone.clone(),
                                ExecState::Failed {
                                    error: format!("invalid response: {}", err),
                                    at: Timestamp::now(),
                                },
                            );
                        }
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id_clone.clone(),
                        ExecState::Failed {
                            error: format!("openagents api {}: {}", status, body),
                            at: Timestamp::now(),
                        },
                    );
                }
                Err(err) => {
                    let mut guard = execs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        exec_id_clone.clone(),
                        ExecState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }
        });

        Ok(exec_id)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        let state = self
            .execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned();
        if let Some(state) = state.as_ref() {
            if !matches!(state, ExecState::Complete(_) | ExecState::Failed { .. }) {
                self.spawn_exec_refresh(exec_id);
            }
        }
        state
    }

    fn poll_exec_output(&self, exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let mut chunk = None;
        {
            let mut guard = self.remote_execs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(exec_id) {
                chunk = state.queue.pop_front();
            }
        }
        if chunk.is_some() {
            return Ok(chunk);
        }
        self.spawn_exec_output_poll(exec_id);
        Ok(None)
    }

    fn cancel_exec(&self, _exec_id: &str) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "cancel_exec".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn read_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _len: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "file_access".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn write_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _data: &[u8],
    ) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "file_access".to_string(),
            provider: self.provider_id.clone(),
        })
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let remote_id = {
            let guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard
                .get(session_id)
                .and_then(|state| state.remote_id.clone())
        }
        .ok_or_else(|| ContainerError::SessionNotFound)?;

        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        sessions.insert(
            session_id.to_string(),
            SessionState::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            },
        );

        let url = self.url(&format!("containers/sessions/{}/stop", remote_id));
        let auth = Arc::clone(&self.auth);
        spawn_local(async move {
            if let Some(token) = auth.token() {
                let _ = wasm_http::request_bytes("POST", &url, Some(&token), None).await;
            }
        });
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let mut chunk = None;
        {
            let mut guard = self
                .remote_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(session_id) {
                chunk = state.queue.pop_front();
            }
        }
        if chunk.is_some() {
            return Ok(chunk);
        }
        self.spawn_session_output_poll(session_id);
        Ok(None)
    }
}

