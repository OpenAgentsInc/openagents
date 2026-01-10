/// Compute provider trait (sync for FileService compatibility).
pub trait ComputeProvider: Send + Sync {
    /// Provider identifier.
    fn id(&self) -> &str;
    /// Provider info (models, pricing, latency, etc.).
    fn info(&self) -> ProviderInfo;
    /// Check if provider is available.
    fn is_available(&self) -> bool;
    /// Check if provider supports a model.
    fn supports_model(&self, model: &str) -> bool;
    /// Submit a compute request. ALWAYS returns job_id immediately.
    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError>;
    /// Get current state of a job by ID.
    fn get_job(&self, job_id: &str) -> Option<JobState>;
    /// Poll streaming job for next chunk (internal use by ComputeFs).
    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError>;
    /// Cancel a running job.
    fn cancel(&self, job_id: &str) -> Result<(), ComputeError>;
}

/// Source for API tokens used by remote compute providers.
pub trait ApiTokenProvider: Send + Sync {
    /// Return the current API token, if available.
    fn api_token(&self) -> Option<String>;
}

/// Routes compute requests to appropriate providers.
#[derive(Default)]
pub struct ComputeRouter {
    providers: Vec<Arc<dyn ComputeProvider>>,
}

impl ComputeRouter {
    /// Create a new router.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Arc<dyn ComputeProvider>) {
        self.providers.push(provider);
    }

    /// List providers.
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }

    /// Get provider by id.
    pub fn provider_by_id(&self, id: &str) -> Option<Arc<dyn ComputeProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    /// Select best provider for request based on policy.
    pub fn select(
        &self,
        request: &ComputeRequest,
        policy: &ComputePolicy,
    ) -> Result<Arc<dyn ComputeProvider>, ComputeError> {
        let candidates: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.is_available())
            .filter(|p| p.supports_model(&request.model))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.contains(&p.id().to_string())
            })
            .filter(|_| {
                policy.allowed_models.is_empty() || policy.allowed_models.contains(&request.model)
            })
            .filter(|_| !policy.blocked_models.contains(&request.model))
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Err(ComputeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "no provider matches policy filters".to_string(),
            });
        }

        let selected = match policy.prefer {
            Prefer::Cost => candidates
                .into_iter()
                .min_by_key(|p| self.estimate_provider_cost_usd(p, request)),
            Prefer::Latency => candidates
                .into_iter()
                .min_by_key(|p| p.info().latency.ttft_ms),
            Prefer::Quality => candidates.into_iter().max_by_key(|p| {
                let info = p.info();
                (
                    info.latency.measured,
                    info.latency.tokens_per_sec.unwrap_or(0),
                )
            }),
            Prefer::Balanced => candidates.into_iter().min_by_key(|p| {
                let cost = self.estimate_provider_cost_usd(p, request);
                let latency = p.info().latency.ttft_ms;
                cost.saturating_mul(latency)
            }),
        };

        selected.ok_or_else(|| ComputeError::NoProviderAvailable {
            model: request.model.clone(),
            reason: "selection failed".to_string(),
        })
    }

    fn estimate_provider_cost_usd(
        &self,
        provider: &Arc<dyn ComputeProvider>,
        request: &ComputeRequest,
    ) -> u64 {
        let info = provider.info();
        if let Some(pricing) = info
            .models
            .iter()
            .find(|model| model.id == request.model)
            .and_then(|model| model.pricing.clone())
        {
            return pricing
                .input_per_1k_microusd
                .saturating_add(pricing.output_per_1k_microusd);
        }
        if let Some(pricing) = info.pricing {
            return pricing
                .input_per_1k_microusd
                .saturating_add(pricing.output_per_1k_microusd);
        }

        request.max_cost_usd.unwrap_or(0)
    }
}

