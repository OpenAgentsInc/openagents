/// Routes container requests to appropriate providers.
#[derive(Default)]
pub struct ContainerRouter {
    providers: Vec<Arc<dyn ContainerProvider>>,
}

impl ContainerRouter {
    /// Create a new router.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Arc<dyn ContainerProvider>) {
        self.providers.push(provider);
    }

    /// List providers.
    pub fn list_providers(&self) -> Vec<ContainerProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }

    /// Get provider by id.
    pub fn provider_by_id(&self, id: &str) -> Option<Arc<dyn ContainerProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    /// Select provider for request based on policy.
    pub fn select(
        &self,
        request: &ContainerRequest,
        policy: &ContainerPolicy,
    ) -> Result<Arc<dyn ContainerProvider>, ContainerError> {
        let candidates: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.is_available())
            .filter(|p| self.image_available(p, &request.image))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.contains(&p.id().to_string())
            })
            .filter(|p| self.within_limits(p, request))
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Err(ContainerError::NoProviderAvailable(
                "no providers match policy".to_string(),
            ));
        }

        let selected = match policy.prefer {
            Prefer::Cost => candidates
                .into_iter()
                .min_by_key(|p| self.estimate_cost_usd(p, request)),
            Prefer::Latency => candidates
                .into_iter()
                .min_by_key(|p| p.info().latency.startup_ms),
            Prefer::Quality => candidates
                .into_iter()
                .max_by_key(|p| p.info().limits.max_memory_mb),
            Prefer::Balanced => candidates.into_iter().min_by_key(|p| {
                let cost = self.estimate_cost_usd(p, request);
                let latency = p.info().latency.startup_ms;
                cost.saturating_mul(latency)
            }),
        };

        selected.ok_or_else(|| {
            ContainerError::NoProviderAvailable("provider selection failed".to_string())
        })
    }

    fn image_available(
        &self,
        provider: &Arc<dyn ContainerProvider>,
        image: &Option<String>,
    ) -> bool {
        let Some(image) = image.as_ref() else {
            return true;
        };
        let info = provider.info();
        if info.available_images.is_empty() {
            return true;
        }
        info.available_images
            .iter()
            .any(|pattern| pattern_matches(pattern, image))
    }

    fn within_limits(
        &self,
        provider: &Arc<dyn ContainerProvider>,
        request: &ContainerRequest,
    ) -> bool {
        let info = provider.info();
        if request.repo.is_some() && !info.capabilities.git_clone {
            return false;
        }
        if matches!(request.kind, ContainerKind::Interactive) && !info.capabilities.interactive {
            return false;
        }
        if request.limits.allow_network && !info.limits.network_allowed {
            return false;
        }
        if request.limits.max_time_secs > info.limits.max_time_secs {
            return false;
        }
        if request.limits.max_memory_mb > info.limits.max_memory_mb {
            return false;
        }
        if request.limits.max_cpu_cores > info.limits.max_cpu_cores {
            return false;
        }
        if request.limits.max_disk_mb > info.limits.max_disk_mb {
            return false;
        }
        true
    }

    fn estimate_cost_usd(
        &self,
        provider: &Arc<dyn ContainerProvider>,
        request: &ContainerRequest,
    ) -> u64 {
        let info = provider.info();
        let pricing = match info.pricing {
            Some(pricing) => pricing,
            None => return request.max_cost_usd.unwrap_or(0),
        };
        let estimated_secs = request.limits.max_time_secs as u64;
        pricing
            .startup_usd
            .saturating_add(pricing.per_second_usd.saturating_mul(estimated_secs))
    }
}

