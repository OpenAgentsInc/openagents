/// Cloudflare Workers AI provider.
#[cfg(feature = "cloudflare")]
pub struct CloudflareProvider {
    ai: Arc<Ai>,
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    last_latency_ms: Arc<Mutex<Option<u64>>>,
}

#[cfg(feature = "cloudflare")]
impl CloudflareProvider {
    /// Create a provider from a Workers AI binding.
    pub fn new(ai: Ai) -> Self {
        Self {
            ai: Arc::new(ai),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            last_latency_ms: Arc::new(Mutex::new(None)),
        }
    }
}

#[cfg(feature = "cloudflare")]
impl ComputeProvider for CloudflareProvider {
    fn id(&self) -> &str {
        "cloudflare"
    }

    fn info(&self) -> ProviderInfo {
        let latency = {
            let guard = self
                .last_latency_ms
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            ProviderLatency {
                ttft_ms: guard.unwrap_or(0),
                tokens_per_sec: None,
                measured: guard.is_some(),
            }
        };
        ProviderInfo {
            id: "cloudflare".to_string(),
            name: "Cloudflare Workers AI".to_string(),
            models: Vec::new(),
            capabilities: Vec::new(),
            pricing: None,
            latency,
            region: None,
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("@cf/")
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let job_id_clone = job_id.clone();
        let ai = Arc::clone(&self.ai);
        let jobs = Arc::clone(&self.jobs);
        let last_latency_ms = Arc::clone(&self.last_latency_ms);
        let request_clone = request.clone();

        self.jobs.write().unwrap_or_else(|e| e.into_inner()).insert(
            job_id.clone(),
            JobState::Pending {
                submitted_at: Timestamp::now(),
            },
        );

        spawn_local(async move {
            {
                let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
                if let Some(job) = jobs.get_mut(&job_id_clone) {
                    *job = JobState::Running {
                        started_at: Timestamp::now(),
                    };
                }
            }

            let start = Instant::now();
            let output: Result<serde_json::Value, worker::Error> = ai
                .run(&request_clone.model, request_clone.input.clone())
                .await;
            let latency_ms = start.elapsed().as_millis() as u64;

            {
                let mut guard = last_latency_ms.lock().unwrap_or_else(|e| e.into_inner());
                *guard = Some(latency_ms);
            }

            let mut jobs = jobs.write().unwrap_or_else(|e| e.into_inner());
            match output {
                Ok(output) => {
                    let cost_usd = request_clone.max_cost_usd.unwrap_or(0); // usage not available; treat reservation as spend
                    let response = ComputeResponse {
                        job_id: job_id_clone.clone(),
                        output,
                        usage: None,
                        cost_usd,
                        latency_ms,
                        provider_id: "cloudflare".to_string(),
                        model: request_clone.model.clone(),
                    };
                    jobs.insert(job_id_clone.clone(), JobState::Complete(response));
                }
                Err(err) => {
                    jobs.insert(
                        job_id_clone.clone(),
                        JobState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        },
                    );
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
            .cloned()
    }

    fn poll_stream(&self, _job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        Ok(None)
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        *job = JobState::Failed {
            error: "cancelled".to_string(),
            at: Timestamp::now(),
        };
        Ok(())
    }
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[derive(Default)]
struct RemoteJobState {
    remote_id: Option<String>,
    cursor: Option<String>,
    queue: VecDeque<ComputeChunk>,
    refreshing: bool,
    streaming: bool,
}

