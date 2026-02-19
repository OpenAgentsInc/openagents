/// OpenAgents API-backed compute provider for browser targets.
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub struct OpenAgentsComputeProvider {
    base_url: String,
    provider_id: String,
    token_provider: Arc<dyn ApiTokenProvider>,
    info: Arc<RwLock<ProviderInfo>>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    remote: Arc<Mutex<HashMap<String, RemoteJobState>>>,
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl OpenAgentsComputeProvider {
    /// Create a new OpenAgents API compute provider.
    pub fn new(
        base_url: impl Into<String>,
        provider_id: impl Into<String>,
        token_provider: Arc<dyn ApiTokenProvider>,
    ) -> Self {
        let provider_id = provider_id.into();
        let info = ProviderInfo {
            id: provider_id.clone(),
            name: format!("OpenAgents ({})", provider_id),
            models: Vec::new(),
            capabilities: Vec::new(),
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 0,
                tokens_per_sec: None,
                measured: false,
            },
            region: Some("openagents".to_string()),
            status: ProviderStatus::Degraded {
                reason: "loading provider info".to_string(),
            },
        };
        let provider = Self {
            base_url: base_url.into(),
            provider_id,
            token_provider,
            info: Arc::new(RwLock::new(info)),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            remote: Arc::new(Mutex::new(HashMap::new())),
        };
        provider.spawn_info_refresh();
        provider
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn spawn_info_refresh(&self) {
        let info = Arc::clone(&self.info);
        let provider_id = self.provider_id.clone();
        let url = self.url(&format!("compute/providers/{}/info", provider_id));
        let token_provider = Arc::clone(&self.token_provider);
        spawn_local(async move {
            let token = token_provider.api_token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let updated = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<ProviderInfo>(&bytes) {
                        Ok(mut info) => {
                            if info.id.is_empty() {
                                info.id = provider_id.clone();
                            }
                            if info.name.is_empty() {
                                info.name = format!("OpenAgents ({})", provider_id);
                            }
                            info
                        }
                        Err(err) => ProviderInfo {
                            id: provider_id.clone(),
                            name: format!("OpenAgents ({})", provider_id),
                            models: Vec::new(),
                            capabilities: Vec::new(),
                            pricing: None,
                            latency: ProviderLatency {
                                ttft_ms: 0,
                                tokens_per_sec: None,
                                measured: false,
                            },
                            region: Some("openagents".to_string()),
                            status: ProviderStatus::Unavailable {
                                reason: format!("invalid provider info: {}", err),
                            },
                        },
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    ProviderInfo {
                        id: provider_id.clone(),
                        name: format!("OpenAgents ({})", provider_id),
                        models: Vec::new(),
                        capabilities: Vec::new(),
                        pricing: None,
                        latency: ProviderLatency {
                            ttft_ms: 0,
                            tokens_per_sec: None,
                            measured: false,
                        },
                        region: Some("openagents".to_string()),
                        status: ProviderStatus::Unavailable {
                            reason: format!("openagents api {}: {}", status, body),
                        },
                    }
                }
                Err(err) => ProviderInfo {
                    id: provider_id.clone(),
                    name: format!("OpenAgents ({})", provider_id),
                    models: Vec::new(),
                    capabilities: Vec::new(),
                    pricing: None,
                    latency: ProviderLatency {
                        ttft_ms: 0,
                        tokens_per_sec: None,
                        measured: false,
                    },
                    region: Some("openagents".to_string()),
                    status: ProviderStatus::Unavailable { reason: err },
                },
            };
            let mut guard = info.write().unwrap_or_else(|e| e.into_inner());
            *guard = updated;
        });
    }

    fn spawn_refresh(&self, job_id: &str) {
        let (remote_id, url, token_provider, jobs, remote, job_id) = {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(job_id) {
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
            let url = self.url(&format!("compute/jobs/{}", remote_id));
            (
                remote_id,
                url,
                Arc::clone(&self.token_provider),
                Arc::clone(&self.jobs),
                Arc::clone(&self.remote),
                job_id.to_string(),
            )
        };

        spawn_local(async move {
            let token = token_provider.api_token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let next_state = match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    serde_json::from_slice::<JobState>(&bytes)
                        .map_err(|err| format!("invalid job state: {}", err))
                }
                Ok((404, _)) => Err("job not found".to_string()),
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    Err(format!("openagents api {}: {}", status, body))
                }
                Err(err) => Err(err),
            };

            match next_state {
                Ok(state) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(job_id.clone(), state);
                }
                Err(err) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id.clone(),
                        JobState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }

            let mut guard = remote.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&job_id) {
                state.refreshing = false;
                state.remote_id = Some(remote_id);
            }
        });
    }

    fn spawn_stream_poll(&self, job_id: &str) {
        let (url, token_provider, jobs, remote, job_id, cursor) = {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            let state = match guard.get_mut(job_id) {
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
                    "compute/jobs/{}/stream?cursor={}",
                    remote_id, cursor
                )),
                None => self.url(&format!("compute/jobs/{}/stream", remote_id)),
            };
            (
                url,
                Arc::clone(&self.token_provider),
                Arc::clone(&self.jobs),
                Arc::clone(&self.remote),
                job_id.to_string(),
                cursor,
            )
        };

        spawn_local(async move {
            let token = token_provider.api_token();
            let response = wasm_http::request_bytes("GET", &url, token.as_deref(), None).await;
            let mut next_chunk: Option<ComputeChunk> = None;
            let mut next_cursor = cursor.clone();
            let mut error: Option<String> = None;

            match response {
                Ok((status, bytes)) if status == 204 || bytes.is_empty() => {}
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    #[derive(Deserialize)]
                    struct StreamResponse {
                        chunk: Option<ComputeChunk>,
                        cursor: Option<String>,
                    }

                    if let Ok(payload) = serde_json::from_slice::<StreamResponse>(&bytes) {
                        next_chunk = payload.chunk;
                        next_cursor = payload.cursor.or(next_cursor);
                    } else if let Ok(chunk) = serde_json::from_slice::<ComputeChunk>(&bytes) {
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
                let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                guard.insert(
                    job_id.clone(),
                    JobState::Failed {
                        error: err,
                        at: Timestamp::now(),
                    },
                );
            } else if let Some(chunk) = next_chunk {
                {
                    let mut guard = remote.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = guard.get_mut(&job_id) {
                        state.queue.push_back(chunk);
                        state.cursor = next_cursor.clone();
                    }
                }
                let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                let updated = match guard.get(&job_id) {
                    Some(JobState::Streaming {
                        started_at,
                        chunks_emitted,
                    }) => JobState::Streaming {
                        started_at: *started_at,
                        chunks_emitted: chunks_emitted.saturating_add(1),
                    },
                    Some(JobState::Running { started_at }) => JobState::Streaming {
                        started_at: *started_at,
                        chunks_emitted: 1,
                    },
                    Some(JobState::Pending { .. }) | None => JobState::Streaming {
                        started_at: Timestamp::now(),
                        chunks_emitted: 1,
                    },
                    Some(JobState::Complete(response)) => JobState::Complete(response.clone()),
                    Some(JobState::Failed { error, at }) => JobState::Failed {
                        error: error.clone(),
                        at: *at,
                    },
                };
                guard.insert(job_id.clone(), updated);
            }

            let mut guard = remote.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(&job_id) {
                state.streaming = false;
                state.cursor = next_cursor;
            }
        });
    }
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
impl ComputeProvider for OpenAgentsComputeProvider {
    fn id(&self) -> &str {
        &self.provider_id
    }

    fn info(&self) -> ProviderInfo {
        self.info.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn is_available(&self) -> bool {
        let info = self.info.read().unwrap_or_else(|e| e.into_inner());
        matches!(
            info.status,
            ProviderStatus::Available | ProviderStatus::Degraded { .. }
        )
    }

    fn supports_model(&self, model: &str) -> bool {
        let info = self.info.read().unwrap_or_else(|e| e.into_inner());
        if info.models.is_empty() {
            return true;
        }
        info.models.iter().any(|entry| entry.id == model)
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let started_at = Timestamp::now();
        self.jobs.write().unwrap_or_else(|e| e.into_inner()).insert(
            job_id.clone(),
            JobState::Pending {
                submitted_at: started_at,
            },
        );
        self.remote
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(job_id.clone(), RemoteJobState::default());

        let jobs = Arc::clone(&self.jobs);
        let remote = Arc::clone(&self.remote);
        let token_provider = Arc::clone(&self.token_provider);
        let url = self.url(&format!("compute/providers/{}/jobs", self.provider_id));
        let job_id_clone = job_id.clone();
        let request_clone = request.clone();

        spawn_local(async move {
            let token = match token_provider.api_token() {
                Some(token) => token,
                None => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: "OpenAgents API token required".to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let body = match serde_json::to_string(&request_clone) {
                Ok(body) => body,
                Err(err) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
                    return;
                }
            };
            let response = wasm_http::request_bytes("POST", &url, Some(&token), Some(body)).await;
            #[derive(Deserialize)]
            struct JobResponse {
                job_id: String,
            }
            match response {
                Ok((status, bytes)) if (200..300).contains(&status) => {
                    match serde_json::from_slice::<JobResponse>(&bytes) {
                        Ok(payload) => {
                            let mut remote_guard = remote.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(state) = remote_guard.get_mut(&job_id_clone) {
                                state.remote_id = Some(payload.job_id);
                            }
                            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                            let state = if request_clone.stream {
                                JobState::Streaming {
                                    started_at,
                                    chunks_emitted: 0,
                                }
                            } else {
                                JobState::Running { started_at }
                            };
                            guard.insert(job_id_clone.clone(), state);
                        }
                        Err(err) => {
                            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                            guard.insert(
                                job_id_clone.clone(),
                                JobState::Failed {
                                    error: format!("invalid response: {}", err),
                                    at: Timestamp::now(),
                                },
                            );
                        }
                    }
                }
                Ok((status, bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: format!("openagents api {}: {}", status, body),
                            at: Timestamp::now(),
                        },
                    );
                }
                Err(err) => {
                    let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: err,
                            at: Timestamp::now(),
                        },
                    );
                }
            }
        });

        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        let state = self
            .jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(job_id)
            .cloned();
        if let Some(state) = state.as_ref() {
            if !matches!(state, JobState::Complete(_) | JobState::Failed { .. }) {
                self.spawn_refresh(job_id);
            }
        }
        state
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut chunk = None;
        {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = guard.get_mut(job_id) {
                chunk = state.queue.pop_front();
            }
        }
        if chunk.is_some() {
            return Ok(chunk);
        }
        self.spawn_stream_poll(job_id);
        Ok(None)
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let remote_id = {
            let mut guard = self.remote.lock().unwrap_or_else(|e| e.into_inner());
            let state = guard.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
            state.remote_id.clone()
        };
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        jobs.insert(
            job_id.to_string(),
            JobState::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            },
        );

        if let Some(remote_id) = remote_id {
            let url = self.url(&format!("compute/jobs/{}/cancel", remote_id));
            let token_provider = Arc::clone(&self.token_provider);
            spawn_local(async move {
                if let Some(token) = token_provider.api_token() {
                    let _ = wasm_http::request_bytes("POST", &url, Some(&token), None).await;
                }
            });
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
const DVM_QUOTE_WINDOW: Duration = Duration::from_secs(5);

