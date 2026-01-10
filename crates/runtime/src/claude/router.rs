/// Claude provider trait (sync for FileService compatibility).
pub trait ClaudeProvider: Send + Sync {
    fn id(&self) -> &str;
    fn info(&self) -> ClaudeProviderInfo;
    fn is_available(&self) -> bool;
    fn supports_model(&self, model: &str) -> bool;
    fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError>;
    fn get_session(&self, session_id: &str) -> Option<SessionState>;
    fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError>;
    fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError>;
    fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError>;
    fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError>;
    fn stop(&self, session_id: &str) -> Result<(), ClaudeError>;
    fn pause(&self, session_id: &str) -> Result<(), ClaudeError>;
    fn resume(&self, session_id: &str) -> Result<(), ClaudeError>;
    fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>>;
    fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo>;
}

/// Routes Claude requests to appropriate providers.
#[derive(Default)]
pub struct ClaudeRouter {
    providers: Vec<Arc<dyn ClaudeProvider>>,
}

impl ClaudeRouter {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn ClaudeProvider>) {
        self.providers.push(provider);
    }

    pub fn list_providers(&self) -> Vec<ClaudeProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }

    pub fn provider_by_id(&self, id: &str) -> Option<Arc<dyn ClaudeProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    pub fn select(
        &self,
        request: &ClaudeRequest,
        policy: &ClaudePolicy,
    ) -> Result<Arc<dyn ClaudeProvider>, ClaudeError> {
        let wants_tunnel = request.tunnel_endpoint.is_some();
        let candidates: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.is_available())
            .filter(|p| p.supports_model(&request.model))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.iter().any(|id| id == p.id())
            })
            .filter(|p| !wants_tunnel || p.id() == "tunnel")
            .filter(|_| {
                policy.allowed_models.is_empty()
                    || policy
                        .allowed_models
                        .iter()
                        .any(|pat| matches_pattern(pat, &request.model))
            })
            .filter(|_| {
                !policy
                    .blocked_models
                    .iter()
                    .any(|pat| matches_pattern(pat, &request.model))
            })
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Err(ClaudeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "no provider matches policy filters".to_string(),
            });
        }

        Ok(candidates
            .into_iter()
            .next()
            .ok_or_else(|| ClaudeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "selection failed".to_string(),
            })?)
    }
}

#[derive(Clone)]
struct SessionRecord {
    provider_id: String,
    reservation: BudgetReservation,
    reconciled: bool,
}

