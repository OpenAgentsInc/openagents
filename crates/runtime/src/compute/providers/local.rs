/// Local provider backed by the compute registry.
#[cfg(not(target_arch = "wasm32"))]
pub struct LocalProvider {
    registry: Arc<BackendRegistry>,
    executor: Executor,
    jobs: Arc<RwLock<HashMap<String, LocalJobState>>>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LocalJobState {
    status: JobState,
    stream_rx: Option<Mutex<mpsc::Receiver<ComputeChunk>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalProvider {
    /// Detect available local backends.
    pub fn detect() -> Result<Self, ComputeError> {
        let executor = Executor::new()?;
        let registry = executor.block_on(async { BackendRegistry::detect().await });
        Ok(Self {
            registry: Arc::new(registry),
            executor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn backend_for_model(
        registry: &BackendRegistry,
        model: &str,
    ) -> Option<Arc<TokioRwLock<dyn InferenceBackend>>> {
        let models = registry.list_all_models().await;
        let backend_id = models
            .into_iter()
            .find(|(_, info)| info.id == model)
            .map(|(id, _)| id);
        backend_id.and_then(|id| registry.get(&id))
    }

    fn map_usage(usage: Option<BackendUsageInfo>) -> Option<TokenUsage> {
        usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens as u64,
            output_tokens: u.completion_tokens as u64,
            total_tokens: u.total_tokens as u64,
        })
    }

    fn completion_request(request: &ComputeRequest) -> Result<CompletionRequest, ComputeError> {
        let prompt = match request.kind {
            ComputeKind::Complete => parse_prompt(&request.input)
                .ok_or_else(|| ComputeError::InvalidRequest("missing prompt".to_string()))?,
            ComputeKind::Chat => parse_messages(&request.input)
                .ok_or_else(|| ComputeError::InvalidRequest("missing messages".to_string()))?,
            _ => return Err(ComputeError::UnsupportedKind(format!("{:?}", request.kind))),
        };

        let mut completion = CompletionRequest::new(request.model.clone(), prompt);
        completion.stream = request.stream;

        if let Some(obj) = request.input.as_object() {
            if let Some(max_tokens) = obj.get("max_tokens").and_then(|v| v.as_u64()) {
                completion.max_tokens = Some(max_tokens as usize);
            }
            if let Some(temp) = obj.get("temperature").and_then(|v| v.as_f64()) {
                completion.temperature = Some(temp as f32);
            }
            if let Some(top_p) = obj.get("top_p").and_then(|v| v.as_f64()) {
                completion.top_p = Some(top_p as f32);
            }
            if let Some(stop) = obj.get("stop").and_then(|v| v.as_array()) {
                let stops: Vec<String> = stop
                    .iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect();
                if !stops.is_empty() {
                    completion.stop = Some(stops);
                }
            }
        }

        Ok(completion)
    }

    fn job_response(
        job_id: &str,
        provider_id: &str,
        response: CompletionResponse,
        latency_ms: u64,
    ) -> ComputeResponse {
        let output = serde_json::json!({
            "text": response.text,
            "finish_reason": response.finish_reason,
        });
        ComputeResponse {
            job_id: job_id.to_string(),
            output,
            usage: Self::map_usage(response.usage),
            cost_usd: 0,
            latency_ms,
            provider_id: provider_id.to_string(),
            model: response.model,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ComputeProvider for LocalProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn info(&self) -> ProviderInfo {
        let models = self
            .executor
            .block_on(async { self.registry.list_all_models().await });
        let has_backends = self.registry.has_backends();
        ProviderInfo {
            id: "local".to_string(),
            name: "Local".to_string(),
            models: models
                .into_iter()
                .map(|(_, model)| ModelInfo {
                    id: model.id,
                    name: model.name,
                    context_length: Some(model.context_length as u32),
                    capabilities: vec![ComputeKind::Chat, ComputeKind::Complete],
                    pricing: None,
                })
                .collect(),
            capabilities: vec![
                ComputeKind::Chat,
                ComputeKind::Complete,
                ComputeKind::Embeddings,
            ],
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 0,
                tokens_per_sec: None,
                measured: false,
            },
            region: Some("local".to_string()),
            status: if has_backends {
                ProviderStatus::Available
            } else {
                ProviderStatus::Unavailable {
                    reason: "no local backend detected".to_string(),
                }
            },
        }
    }

    fn is_available(&self) -> bool {
        self.registry.has_backends()
    }

    fn supports_model(&self, model: &str) -> bool {
        self.executor.block_on(async {
            let models = self.registry.list_all_models().await;
            models.iter().any(|(_, info)| info.id == model)
        })
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let stream = request.stream;
        let provider_id = self.id().to_string();
        let start = Instant::now();
        let jobs = self.jobs.clone();
        let registry = self.registry.clone();
        let executor = self.executor.clone();
        let request_clone = request.clone();
        let job_id_clone = job_id.clone();

        let (stream_tx, stream_rx) = if stream {
            let (tx, rx) = mpsc::channel(64);
            (Some(tx), Some(Mutex::new(rx)))
        } else {
            (None, None)
        };

        self.jobs.write().unwrap_or_else(|e| e.into_inner()).insert(
            job_id.clone(),
            LocalJobState {
                status: JobState::Pending {
                    submitted_at: Timestamp::now(),
                },
                stream_rx,
            },
        );

        executor.spawn(async move {
            let backend = LocalProvider::backend_for_model(&registry, &request_clone.model).await;
            let backend = match backend {
                Some(backend) => backend,
                None => {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Failed {
                            error: "model not available".to_string(),
                            at: Timestamp::now(),
                        };
                    }
                    return;
                }
            };

            let completion_request = match LocalProvider::completion_request(&request_clone) {
                Ok(req) => req,
                Err(err) => {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        };
                    }
                    return;
                }
            };

            if stream {
                {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Streaming {
                            started_at: Timestamp::now(),
                            chunks_emitted: 0,
                        };
                    }
                }
                let rx = {
                    let backend = backend.read().await;
                    match backend.complete_stream(completion_request).await {
                        Ok(rx) => rx,
                        Err(err) => {
                            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                            if let Some(job) = jobs.get_mut(&job_id_clone) {
                                job.status = JobState::Failed {
                                    error: err.to_string(),
                                    at: Timestamp::now(),
                                };
                            }
                            return;
                        }
                    }
                };

                let mut output_text = String::new();
                let mut finish_reason: Option<String> = None;
                let mut stream = rx;
                while let Some(chunk) = stream.recv().await {
                    match chunk {
                        Ok(chunk) => {
                            output_text.push_str(&chunk.delta);
                            finish_reason = chunk.finish_reason.clone();
                            if let Some(tx) = stream_tx.as_ref() {
                                let compute_chunk = ComputeChunk {
                                    job_id: job_id_clone.clone(),
                                    delta: serde_json::json!({ "text": chunk.delta }),
                                    finish_reason: chunk.finish_reason.clone(),
                                    usage: None,
                                };
                                let _ = tx.send(compute_chunk).await;
                            }
                            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                            if let Some(job) = jobs.get_mut(&job_id_clone) {
                                if let JobState::Streaming {
                                    started_at,
                                    chunks_emitted,
                                } = job.status.clone()
                                {
                                    job.status = JobState::Streaming {
                                        started_at,
                                        chunks_emitted: chunks_emitted.saturating_add(1),
                                    };
                                }
                            }
                        }
                        Err(err) => {
                            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                            if let Some(job) = jobs.get_mut(&job_id_clone) {
                                job.status = JobState::Failed {
                                    error: err.to_string(),
                                    at: Timestamp::now(),
                                };
                            }
                            return;
                        }
                    }
                }

                let response = ComputeResponse {
                    job_id: job_id_clone.clone(),
                    output: serde_json::json!({
                        "text": output_text,
                        "finish_reason": finish_reason,
                    }),
                    usage: None,
                    cost_usd: 0,
                    latency_ms: start.elapsed().as_millis() as u64,
                    provider_id: provider_id.clone(),
                    model: request_clone.model.clone(),
                };
                let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                if let Some(job) = jobs.get_mut(&job_id_clone) {
                    job.status = JobState::Complete(response);
                }
                return;
            }

            let completion = {
                let backend = backend.read().await;
                backend.complete(completion_request).await
            };
            match completion {
                Ok(response) => {
                    let response = LocalProvider::job_response(
                        &job_id_clone,
                        &provider_id,
                        response,
                        start.elapsed().as_millis() as u64,
                    );
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Complete(response);
                    }
                }
                Err(err) => {
                    let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        };
                    }
                }
            }
        });

        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(job_id)
            .map(|job| job.status.clone())
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        let rx = match job.stream_rx.as_mut() {
            Some(rx) => rx,
            None => return Ok(None),
        };
        let mut rx = rx.lock().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(chunk) => Ok(Some(chunk)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        job.status = JobState::Failed {
            error: "cancelled".to_string(),
            at: Timestamp::now(),
        };
        Ok(())
    }
}

