/// Container filesystem service.
pub struct ContainerFs {
    agent_id: AgentId,
    router: Arc<RwLock<ContainerRouter>>,
    policy: Arc<RwLock<ContainerPolicy>>,
    auth: Arc<OpenAgentsAuth>,
    budget: Arc<Mutex<BudgetTracker>>,
    journal: Arc<dyn IdempotencyJournal>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    execs: Arc<RwLock<HashMap<String, ExecRecord>>>,
}

impl ContainerFs {
    /// Create a new container filesystem.
    pub fn new(
        agent_id: AgentId,
        router: ContainerRouter,
        policy: ContainerPolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        let auth = Arc::new(OpenAgentsAuth::from_env(agent_id.clone(), storage, signer));
        Self::with_auth(agent_id, router, policy, budget_policy, journal, auth)
    }

    /// Create a container filesystem with local + OpenAgents providers from env.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_default_providers(
        agent_id: AgentId,
        policy: ContainerPolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        let api = openagents_api_from_env();
        let auth = Arc::new(OpenAgentsAuth::new(
            agent_id.clone(),
            storage,
            signer,
            api.clone(),
        ));
        let mut router = ContainerRouter::new();
        #[cfg(all(target_os = "macos", feature = "apple-container"))]
        router.register(Arc::new(AppleContainerProvider::new()));
        router.register(Arc::new(LocalContainerProvider::new()));
        if let Some(api) = api {
            router.register(Arc::new(OpenAgentsContainerProvider::cloudflare(
                api.clone(),
                auth.clone(),
            )));
            router.register(Arc::new(OpenAgentsContainerProvider::daytona(
                api,
                auth.clone(),
            )));
        }
        Self::with_auth(agent_id, router, policy, budget_policy, journal, auth)
    }

    /// Create a container filesystem with a preconfigured auth manager.
    pub fn with_auth(
        agent_id: AgentId,
        router: ContainerRouter,
        policy: ContainerPolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        Self {
            agent_id,
            router: Arc::new(RwLock::new(router)),
            policy: Arc::new(RwLock::new(policy)),
            auth,
            budget: Arc::new(Mutex::new(BudgetTracker::new(budget_policy))),
            journal,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn usage_json(&self) -> FsResult<Vec<u8>> {
        let tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        let policy = tracker.policy().clone();
        let state = tracker.state().clone();
        let json = serde_json::json!({
            "tick": {
                "reserved_usd": state.reserved_tick_usd,
                "spent_usd": state.spent_tick_usd,
                "limit_usd": policy.per_tick_usd,
                "remaining_usd": state.remaining_tick(&policy),
            },
            "day": {
                "reserved_usd": state.reserved_day_usd,
                "spent_usd": state.spent_day_usd,
                "limit_usd": policy.per_day_usd,
                "remaining_usd": state.remaining_day(&policy),
            }
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn reconcile_session(&self, session_id: &str, state: &SessionState) -> FsResult<()> {
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let record = match sessions.get_mut(session_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }
        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            SessionState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
                if record.credits_reserved > 0 {
                    self.auth
                        .reconcile_credits(record.credits_reserved, response.cost_usd)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            SessionState::Failed { .. } | SessionState::Expired { .. } => {
                tracker.release(record.reservation);
                if record.credits_reserved > 0 {
                    self.auth
                        .release_credits(record.credits_reserved)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }

    fn session_provider(&self, session_id: &str) -> FsResult<Arc<dyn ContainerProvider>> {
        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
        let record = sessions.get(session_id).ok_or(FsError::NotFound)?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)
    }

    fn exec_provider(&self, exec_id: &str) -> FsResult<(Arc<dyn ContainerProvider>, ExecRecord)> {
        let execs = self.execs.read().unwrap_or_else(|e| e.into_inner());
        let record = execs.get(exec_id).ok_or(FsError::NotFound)?.clone();
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)?;
        Ok((provider, record))
    }
}

impl FileService for ContainerFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            ["new"] if flags.write => Ok(Box::new(ContainerNewHandle::new(
                self.agent_id.clone(),
                self.router.clone(),
                self.policy.clone(),
                self.auth.clone(),
                self.budget.clone(),
                self.sessions.clone(),
                self.journal.clone(),
            ))),
            ["policy"] => {
                if flags.write {
                    Ok(Box::new(PolicyWriteHandle::new(self.policy.clone())))
                } else {
                    let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                    let json = serde_json::to_vec_pretty(&*policy)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    Ok(Box::new(BytesHandle::new(json)))
                }
            }
            ["usage"] => Ok(Box::new(BytesHandle::new(self.usage_json()?))),
            ["auth", "status"] => Ok(Box::new(BytesHandle::new(self.auth.status_json()?))),
            ["auth", "credits"] => Ok(Box::new(BytesHandle::new(self.auth.credits_json()?))),
            ["auth", "token"] => {
                if flags.write {
                    Ok(Box::new(AuthTokenHandle::new(self.auth.clone())))
                } else {
                    Err(FsError::PermissionDenied)
                }
            }
            ["auth", "challenge"] => {
                if flags.write {
                    Ok(Box::new(AuthChallengeWriteHandle::new(self.auth.clone())))
                } else {
                    Ok(Box::new(BytesHandle::new(self.auth.challenge_json()?)))
                }
            }
            ["providers"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let providers = router.list_providers();
                let json = serde_json::to_vec_pretty(&providers)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "info"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "images"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info.available_images)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "health"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::json!({
                    "status": match info.status {
                        ProviderStatus::Available => "available",
                        ProviderStatus::Degraded { .. } => "degraded",
                        ProviderStatus::Unavailable { .. } => "unavailable",
                    }
                });
                let bytes =
                    serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "status"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
                self.reconcile_session(session_id, &state)?;
                let (status, error) = match &state {
                    SessionState::Provisioning { .. } => ("provisioning", None),
                    SessionState::Cloning { .. } => ("cloning", None),
                    SessionState::Running { .. } => ("running", None),
                    SessionState::Complete(_) => ("complete", None),
                    SessionState::Failed { error, .. } => ("failed", Some(error.clone())),
                    SessionState::Expired { .. } => ("expired", None),
                };
                let json = serde_json::json!({
                    "status": status,
                    "error": error,
                });
                let bytes =
                    serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "result"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
                self.reconcile_session(session_id, &state)?;
                match state {
                    SessionState::Complete(response) => {
                        let json = serde_json::to_vec_pretty(&response)
                            .map_err(|err| FsError::Other(err.to_string()))?;
                        Ok(Box::new(BytesHandle::new(json)))
                    }
                    SessionState::Failed { error, .. } => Err(FsError::Other(error)),
                    _ => Err(FsError::Other("not ready".to_string())),
                }
            }
            ["sessions", session_id, "usage"] => {
                let provider = self.session_provider(session_id)?;
                let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
                self.reconcile_session(session_id, &state)?;
                let response = match state {
                    SessionState::Complete(response) => serde_json::json!({
                        "usage": response.usage,
                        "cost_usd": response.cost_usd,
                        "reserved_usd": response.reserved_usd,
                        "duration_ms": response.duration_ms,
                    }),
                    _ => serde_json::json!({
                        "usage": ContainerUsage::zero(),
                        "cost_usd": 0,
                        "reserved_usd": 0,
                        "duration_ms": 0,
                    }),
                };
                let bytes =
                    serde_json::to_vec(&response).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "ctl"] if flags.write => Ok(Box::new(CtlHandle::new(
                session_id.to_string(),
                self.router.clone(),
                self.sessions.clone(),
            ))),
            ["sessions", session_id, "exec", "new"] if flags.write => {
                Ok(Box::new(ExecNewHandle::new(
                    session_id.to_string(),
                    self.router.clone(),
                    self.execs.clone(),
                )))
            }
            ["sessions", session_id, "exec", exec_id, "status"] => {
                let (provider, record) = self.exec_provider(exec_id)?;
                if record.session_id != *session_id {
                    return Err(FsError::NotFound);
                }
                let state = provider.get_exec(exec_id).ok_or(FsError::NotFound)?;
                let (status, error) = match &state {
                    ExecState::Pending { .. } => ("pending", None),
                    ExecState::Running { .. } => ("running", None),
                    ExecState::Complete(_) => ("complete", None),
                    ExecState::Failed { error, .. } => ("failed", Some(error.clone())),
                };
                let json = serde_json::json!({
                    "status": status,
                    "error": error,
                });
                let bytes =
                    serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(bytes)))
            }
            ["sessions", session_id, "exec", exec_id, "result"] => {
                let (provider, record) = self.exec_provider(exec_id)?;
                if record.session_id != *session_id {
                    return Err(FsError::NotFound);
                }
                let state = provider.get_exec(exec_id).ok_or(FsError::NotFound)?;
                match state {
                    ExecState::Complete(result) => {
                        let json = serde_json::to_vec_pretty(&result)
                            .map_err(|err| FsError::Other(err.to_string()))?;
                        Ok(Box::new(BytesHandle::new(json)))
                    }
                    ExecState::Failed { error, .. } => Err(FsError::Other(error)),
                    _ => Err(FsError::Other("not ready".to_string())),
                }
            }
            ["sessions", session_id, "files", encoded] => {
                let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                let file_path = decode_path(encoded)?;
                validate_relative_path(&file_path)?;
                let provider = self.session_provider(session_id)?;
                if flags.write {
                    Ok(Box::new(FileWriteHandle::new(
                        provider,
                        session_id.to_string(),
                        file_path,
                        0,
                        policy.max_file_size_bytes,
                        false,
                    )))
                } else {
                    let data = provider
                        .read_file(
                            session_id,
                            &file_path,
                            0,
                            policy.max_file_size_bytes.saturating_add(1),
                        )
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    if data.len() as u64 > policy.max_file_size_bytes {
                        return Err(FsError::Other("file too large".to_string()));
                    }
                    Ok(Box::new(BytesHandle::new(data)))
                }
            }
            ["sessions", session_id, "files", encoded, "chunks", chunk] => {
                let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                let file_path = decode_path(encoded)?;
                validate_relative_path(&file_path)?;
                let chunk_index: u64 = chunk.parse().map_err(|_| FsError::InvalidPath)?;
                let offset = chunk_index.saturating_mul(CHUNK_SIZE);
                let provider = self.session_provider(session_id)?;
                if flags.write {
                    let max_chunk = policy.max_file_size_bytes.min(CHUNK_SIZE);
                    Ok(Box::new(FileWriteHandle::new(
                        provider,
                        session_id.to_string(),
                        file_path,
                        offset,
                        max_chunk,
                        true,
                    )))
                } else {
                    let len = CHUNK_SIZE.min(policy.max_file_size_bytes);
                    let data = provider
                        .read_file(session_id, &file_path, offset, len)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    Ok(Box::new(BytesHandle::new(data)))
                }
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        let path = path.trim_matches('/');
        match path {
            "" => Ok(vec![
                DirEntry::dir("providers"),
                DirEntry::file("new", 0),
                DirEntry::file("policy", 0),
                DirEntry::file("usage", 0),
                DirEntry::dir("auth"),
                DirEntry::dir("sessions"),
            ]),
            "auth" => Ok(vec![
                DirEntry::file("status", 0),
                DirEntry::file("token", 0),
                DirEntry::file("challenge", 0),
                DirEntry::file("credits", 0),
            ]),
            "providers" => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                Ok(router
                    .list_providers()
                    .iter()
                    .map(|p| DirEntry::dir(&p.id))
                    .collect())
            }
            "sessions" => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                Ok(sessions.keys().map(|id| DirEntry::dir(id)).collect())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            [] => Ok(Stat::dir()),
            ["providers"] | ["sessions"] | ["auth"] => Ok(Stat::dir()),
            ["new"] | ["policy"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions::read_write(),
            }),
            ["usage"] => Ok(Stat::file(0)),
            ["auth", "status"] | ["auth", "credits"] => Ok(Stat::file(0)),
            ["auth", "challenge"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions::read_write(),
            }),
            ["auth", "token"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions {
                    read: false,
                    write: true,
                    execute: false,
                },
            }),
            ["providers", id] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["providers", id, "info"]
            | ["providers", id, "images"]
            | ["providers", id, "health"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "status"]
            | ["sessions", session_id, "result"]
            | ["sessions", session_id, "output"]
            | ["sessions", session_id, "usage"]
            | ["sessions", session_id, "ctl"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec", "new"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat {
                        size: 0,
                        is_dir: false,
                        created: None,
                        modified: None,
                        permissions: Permissions::read_write(),
                    })
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec", exec_id] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                let execs = self.execs.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) && execs.contains_key(*exec_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "exec", exec_id, "status"]
            | ["sessions", session_id, "exec", exec_id, "result"]
            | ["sessions", session_id, "exec", exec_id, "output"] => {
                let execs = self.execs.read().unwrap_or_else(|e| e.into_inner());
                if execs
                    .get(*exec_id)
                    .map(|record| record.session_id == *session_id)
                    .unwrap_or(false)
                {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "files", ..] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn mkdir(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn remove(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn rename(&self, _from: &str, _to: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn watch(&self, path: &str) -> FsResult<Option<Box<dyn WatchHandle>>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if let ["sessions", session_id, "output"] = parts.as_slice() {
            let provider = self.session_provider(session_id)?;
            return Ok(Some(Box::new(SessionWatchHandle::new(
                session_id.to_string(),
                provider,
                self.sessions.clone(),
                self.budget.clone(),
                self.auth.clone(),
            ))));
        }
        if let ["sessions", session_id, "exec", exec_id, "output"] = parts.as_slice() {
            let (provider, record) = self.exec_provider(exec_id)?;
            if record.session_id != *session_id {
                return Err(FsError::NotFound);
            }
            return Ok(Some(Box::new(ExecWatchHandle::new(
                exec_id.to_string(),
                provider,
            ))));
        }
        Ok(None)
    }

    fn name(&self) -> &str {
        "containers"
    }
}

struct ContainerNewHandle {
    agent_id: AgentId,
    router: Arc<RwLock<ContainerRouter>>,
    policy: Arc<RwLock<ContainerPolicy>>,
    auth: Arc<OpenAgentsAuth>,
    budget: Arc<Mutex<BudgetTracker>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ContainerNewHandle {
    fn new(
        agent_id: AgentId,
        router: Arc<RwLock<ContainerRouter>>,
        policy: Arc<RwLock<ContainerPolicy>>,
        auth: Arc<OpenAgentsAuth>,
        budget: Arc<Mutex<BudgetTracker>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router,
            policy,
            auth,
            budget,
            sessions,
            journal,
            request_buf: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit_request(&mut self) -> FsResult<()> {
        let mut request: ContainerRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let policy = self
            .policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(FsError::Other(
                ContainerError::IdempotencyRequired.to_string(),
            ));
        }

        if request.timeout_ms.is_none() {
            request.timeout_ms = Some(default_timeout_ms());
        }

        if request.commands.is_empty() && !matches!(request.kind, ContainerKind::Interactive) {
            return Err(FsError::Other("commands required".to_string()));
        }

        validate_image(&policy, &request.image)?;
        validate_limits(&policy, &request.limits)?;

        if policy.max_concurrent > 0 {
            let active = count_active_sessions(&self.router, &self.sessions);
            if active as u32 >= policy.max_concurrent {
                return Err(FsError::Other(
                    "max concurrent containers reached".to_string(),
                ));
            }
        }

        let max_cost_usd = match (
            request.max_cost_usd,
            policy.default_max_cost_usd,
            policy.require_max_cost,
        ) {
            (Some(cost), _, _) => cost,
            (None, Some(default_cost), _) => {
                request.max_cost_usd = Some(default_cost);
                default_cost
            }
            (None, None, true) => {
                return Err(FsError::Other(ContainerError::MaxCostRequired.to_string()));
            }
            (None, None, false) => {
                let tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                let budget_policy = tracker.policy();
                if budget_policy.per_tick_usd > 0 {
                    budget_policy.per_tick_usd
                } else {
                    budget_policy.per_day_usd
                }
            }
        };

        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .select(&request, &policy)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let provider_id = provider.id().to_string();

        let requires_auth = provider.requires_openagents_auth();
        self.auth
            .check_auth(&provider_id, &policy, requires_auth)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let requires_credits = requires_auth;

        let scoped_key = request
            .idempotency_key
            .as_ref()
            .map(|key| format!("{}:{}:{}", self.agent_id.as_str(), provider_id, key));

        if let Some(key) = scoped_key.as_ref() {
            if let Some(cached) = self
                .journal
                .get(key)
                .map_err(|err| FsError::Other(err.to_string()))?
            {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&cached) {
                    if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
                        self.sessions
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry(session_id.to_string())
                            .or_insert(SessionRecord {
                                provider_id: provider_id.clone(),
                                reservation: BudgetReservation { amount_usd: 0 },
                                reconciled: true,
                                credits_reserved: 0,
                            });
                    }
                }
                self.response = Some(cached);
                return Ok(());
            }
        }

        if requires_credits {
            self.auth
                .check_credits(max_cost_usd)
                .map_err(|err| FsError::Other(err.to_string()))?;
        }

        let reservation = {
            let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
            let reservation = tracker
                .reserve(max_cost_usd)
                .map_err(|_| FsError::BudgetExceeded)?;
            let state = tracker.state().clone();
            if let Some(limit) = policy.max_cost_usd_per_tick {
                if state.reserved_tick_usd + state.spent_tick_usd > limit {
                    tracker.release(reservation);
                    return Err(FsError::BudgetExceeded);
                }
            }
            if let Some(limit) = policy.max_cost_usd_per_day {
                if state.reserved_day_usd + state.spent_day_usd > limit {
                    tracker.release(reservation);
                    return Err(FsError::BudgetExceeded);
                }
            }
            reservation
        };

        let credits_reserved = if requires_credits {
            match self.auth.reserve_credits(max_cost_usd) {
                Ok(reserved) => reserved,
                Err(err) => {
                    let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                    tracker.release(reservation);
                    return Err(FsError::Other(err.to_string()));
                }
            }
        } else {
            0
        };

        let session_id = match provider.submit(request.clone()) {
            Ok(session_id) => session_id,
            Err(err) => {
                let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                tracker.release(reservation);
                if credits_reserved > 0 {
                    let _ = self.auth.release_credits(credits_reserved);
                }
                return Err(FsError::Other(err.to_string()));
            }
        };

        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                SessionRecord {
                    provider_id,
                    reservation,
                    reconciled: false,
                    credits_reserved,
                },
            );

        let response_json = serde_json::json!({
            "session_id": session_id,
            "status": "provisioning",
            "status_path": format!("/containers/sessions/{}/status", session_id),
            "output_path": format!("/containers/sessions/{}/output", session_id),
            "result_path": format!("/containers/sessions/{}/result", session_id),
            "exec_path": format!("/containers/sessions/{}/exec", session_id),
            "files_path": format!("/containers/sessions/{}/files", session_id),
        });
        let response_bytes =
            serde_json::to_vec(&response_json).map_err(|err| FsError::Other(err.to_string()))?;

        if let Some(key) = scoped_key.as_ref() {
            self.journal
                .put_with_ttl(key, &response_bytes, IDEMPOTENCY_TTL)
                .map_err(|err| FsError::Other(err.to_string()))?;
        }

        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ContainerNewHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.submit_request()?;
        }
        let response = self.response.as_ref().unwrap();
        if self.position >= response.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), response.len() - self.position);
        buf[..len].copy_from_slice(&response[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.request_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        if self.response.is_none() {
            return Err(FsError::InvalidPath);
        }
        let response = self.response.as_ref().unwrap();
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => response.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.response.is_none() && !self.request_buf.is_empty() {
            self.submit_request()?;
        }
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct PolicyWriteHandle {
    policy: Arc<RwLock<ContainerPolicy>>,
    buffer: Vec<u8>,
}

impl PolicyWriteHandle {
    fn new(policy: Arc<RwLock<ContainerPolicy>>) -> Self {
        Self {
            policy,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for PolicyWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let policy: ContainerPolicy =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        let mut guard = self.policy.write().unwrap_or_else(|e| e.into_inner());
        *guard = policy;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct AuthTokenHandle {
    auth: Arc<OpenAgentsAuth>,
    buffer: Vec<u8>,
}

impl AuthTokenHandle {
    fn new(auth: Arc<OpenAgentsAuth>) -> Self {
        Self {
            auth,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for AuthTokenHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let token = String::from_utf8(self.buffer.clone())
            .map_err(|_| FsError::Other("invalid token utf-8".to_string()))?;
        if token.trim().is_empty() {
            return Err(FsError::Other("token required".to_string()));
        }
        self.auth
            .set_token(token.trim())
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct AuthChallengeWriteHandle {
    auth: Arc<OpenAgentsAuth>,
    buffer: Vec<u8>,
}

impl AuthChallengeWriteHandle {
    fn new(auth: Arc<OpenAgentsAuth>) -> Self {
        Self {
            auth,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for AuthChallengeWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let response: NostrAuthResponse =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        self.auth
            .submit_challenge(response)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ExecNewHandle {
    session_id: String,
    router: Arc<RwLock<ContainerRouter>>,
    execs: Arc<RwLock<HashMap<String, ExecRecord>>>,
    buffer: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ExecNewHandle {
    fn new(
        session_id: String,
        router: Arc<RwLock<ContainerRouter>>,
        execs: Arc<RwLock<HashMap<String, ExecRecord>>>,
    ) -> Self {
        Self {
            session_id,
            router,
            execs,
            buffer: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit(&mut self) -> FsResult<()> {
        let command = String::from_utf8(self.buffer.clone())
            .map_err(|_| FsError::Other("invalid utf-8 command".to_string()))?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        let exec_id = provider
            .submit_exec(&self.session_id, command.trim())
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                ExecRecord {
                    provider_id: provider.id().to_string(),
                    session_id: self.session_id.clone(),
                },
            );
        let response_json = serde_json::json!({
            "exec_id": exec_id,
            "status": "pending",
            "status_path": format!("/containers/sessions/{}/exec/{}/status", self.session_id, exec_id),
            "output_path": format!("/containers/sessions/{}/exec/{}/output", self.session_id, exec_id),
            "result_path": format!("/containers/sessions/{}/exec/{}/result", self.session_id, exec_id),
        });
        let response_bytes =
            serde_json::to_vec(&response_json).map_err(|err| FsError::Other(err.to_string()))?;
        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ExecNewHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.submit()?;
        }
        let response = self.response.as_ref().unwrap();
        if self.position >= response.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), response.len() - self.position);
        buf[..len].copy_from_slice(&response[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        if self.response.is_none() {
            return Err(FsError::InvalidPath);
        }
        let response = self.response.as_ref().unwrap();
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => response.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.response.is_none() && !self.buffer.is_empty() {
            self.submit()?;
        }
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct CtlHandle {
    session_id: String,
    router: Arc<RwLock<ContainerRouter>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    buffer: Vec<u8>,
}

impl CtlHandle {
    fn new(
        session_id: String,
        router: Arc<RwLock<ContainerRouter>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    ) -> Self {
        Self {
            session_id,
            router,
            sessions,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for CtlHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        let command = String::from_utf8_lossy(&self.buffer);
        if command.trim() != "stop" {
            return Err(FsError::Other("unsupported ctl command".to_string()));
        }
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        provider
            .stop(&self.session_id)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.session_id);
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct SessionWatchHandle {
    session_id: String,
    provider: Arc<dyn ContainerProvider>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    budget: Arc<Mutex<BudgetTracker>>,
    auth: Arc<OpenAgentsAuth>,
}

impl SessionWatchHandle {
    fn new(
        session_id: String,
        provider: Arc<dyn ContainerProvider>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        budget: Arc<Mutex<BudgetTracker>>,
        auth: Arc<OpenAgentsAuth>,
    ) -> Self {
        Self {
            session_id,
            provider,
            sessions,
            budget,
            auth,
        }
    }

    fn reconcile(&self, state: &SessionState) -> FsResult<()> {
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let record = match sessions.get_mut(&self.session_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }
        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            SessionState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
                if record.credits_reserved > 0 {
                    self.auth
                        .reconcile_credits(record.credits_reserved, response.cost_usd)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            SessionState::Failed { .. } | SessionState::Expired { .. } => {
                tracker.release(record.reservation);
                if record.credits_reserved > 0 {
                    self.auth
                        .release_credits(record.credits_reserved)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                }
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }
}

impl WatchHandle for SessionWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            match self.provider.poll_output(&self.session_id) {
                Ok(Some(chunk)) => {
                    if let Some(state) = self.provider.get_session(&self.session_id) {
                        self.reconcile(&state)?;
                    }
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = self.provider.get_session(&self.session_id) {
                        if matches!(
                            state,
                            SessionState::Complete(_)
                                | SessionState::Failed { .. }
                                | SessionState::Expired { .. }
                        ) {
                            self.reconcile(&state)?;
                            return Ok(None);
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }
            if !wait_for_output(deadline)? {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct ExecWatchHandle {
    exec_id: String,
    provider: Arc<dyn ContainerProvider>,
}

impl ExecWatchHandle {
    fn new(exec_id: String, provider: Arc<dyn ContainerProvider>) -> Self {
        Self { exec_id, provider }
    }
}

impl WatchHandle for ExecWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            match self.provider.poll_exec_output(&self.exec_id) {
                Ok(Some(chunk)) => {
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = self.provider.get_exec(&self.exec_id) {
                        if matches!(state, ExecState::Complete(_) | ExecState::Failed { .. }) {
                            return Ok(None);
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }
            if !wait_for_output(deadline)? {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct FileWriteHandle {
    provider: Arc<dyn ContainerProvider>,
    session_id: String,
    path: String,
    offset: u64,
    buffer: Vec<u8>,
    max_size: u64,
    is_chunk: bool,
}

fn wait_for_output(deadline: Option<Instant>) -> FsResult<bool> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = deadline;
        return Ok(false);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                return Ok(false);
            }
        }
        thread::sleep(Duration::from_millis(25));
        Ok(true)
    }
}

impl FileWriteHandle {
    fn new(
        provider: Arc<dyn ContainerProvider>,
        session_id: String,
        path: String,
        offset: u64,
        max_size: u64,
        is_chunk: bool,
    ) -> Self {
        Self {
            provider,
            session_id,
            path,
            offset,
            buffer: Vec::new(),
            max_size,
            is_chunk,
        }
    }
}

impl FileHandle for FileWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        if (self.buffer.len() + buf.len()) as u64 > self.max_size {
            return Err(FsError::Other("file write exceeds max size".to_string()));
        }
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        if self.is_chunk && self.buffer.len() as u64 > CHUNK_SIZE {
            return Err(FsError::Other("chunk exceeds max chunk size".to_string()));
        }
        self.provider
            .write_file(&self.session_id, &self.path, self.offset, &self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

fn decode_path(encoded: &str) -> FsResult<String> {
    if encoded.len() > MAX_PATH_LEN {
        return Err(FsError::InvalidPath);
    }
    decode(encoded)
        .map(|value| value.into_owned())
        .map_err(|_| FsError::InvalidPath)
}

fn validate_relative_path(path: &str) -> FsResult<()> {
    if path.is_empty() || path.len() > MAX_PATH_LEN {
        return Err(FsError::InvalidPath);
    }
    if path.starts_with('/') {
        return Err(FsError::InvalidPath);
    }
    if path.contains('\\') {
        return Err(FsError::InvalidPath);
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(FsError::InvalidPath);
        }
    }
    Ok(())
}

fn validate_image(policy: &ContainerPolicy, image: &Option<String>) -> FsResult<()> {
    let Some(image) = image.as_ref() else {
        return Ok(());
    };
    if policy
        .blocked_images
        .iter()
        .any(|pattern| pattern_matches(pattern, image))
    {
        return Err(FsError::Other("image blocked by policy".to_string()));
    }
    if !policy.allowed_images.is_empty()
        && !policy
            .allowed_images
            .iter()
            .any(|pattern| pattern_matches(pattern, image))
    {
        return Err(FsError::Other("image not allowed by policy".to_string()));
    }
    Ok(())
}

fn validate_limits(policy: &ContainerPolicy, limits: &ResourceLimits) -> FsResult<()> {
    if !policy.allow_network && limits.allow_network {
        return Err(FsError::Other("network access not allowed".to_string()));
    }
    if limits.max_time_secs > policy.max_execution_time_secs {
        return Err(FsError::Other("max execution time exceeded".to_string()));
    }
    if limits.max_memory_mb > policy.max_memory_mb {
        return Err(FsError::Other("max memory exceeded".to_string()));
    }
    Ok(())
}

fn count_active_sessions(
    router: &Arc<RwLock<ContainerRouter>>,
    sessions: &Arc<RwLock<HashMap<String, SessionRecord>>>,
) -> usize {
    let sessions = sessions.read().unwrap_or_else(|e| e.into_inner());
    let router = router.read().unwrap_or_else(|e| e.into_inner());
    sessions
        .iter()
        .filter(|(session_id, record)| {
            if let Some(provider) = router.provider_by_id(&record.provider_id) {
                match provider.get_session(session_id) {
                    Some(SessionState::Complete(_))
                    | Some(SessionState::Failed { .. })
                    | Some(SessionState::Expired { .. }) => false,
                    Some(_) => true,
                    None => true,
                }
            } else {
                true
            }
        })
        .count()
}

fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut pos = 0usize;
    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some(found) = value[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
        if idx == 0 && !pattern.starts_with('*') && !value.starts_with(part) {
            return false;
        }
    }
    if !pattern.ends_with('*') {
        if let Some(last) = parts.last() {
            if !value.ends_with(last) {
                return false;
            }
        }
    }
    true
}

fn unavailable_provider_info(id: &str, name: &str, reason: String) -> ContainerProviderInfo {
    ContainerProviderInfo {
        id: id.to_string(),
        name: name.to_string(),
        available_images: Vec::new(),
        capabilities: ContainerCapabilities {
            git_clone: false,
            file_access: false,
            interactive: false,
            artifacts: false,
            streaming: false,
        },
        pricing: None,
        latency: ContainerLatency {
            startup_ms: 0,
            measured: false,
        },
        limits: ContainerLimits {
            max_memory_mb: 0,
            max_cpu_cores: 0.0,
            max_disk_mb: 0,
            max_time_secs: 0,
            network_allowed: false,
        },
        status: ProviderStatus::Unavailable { reason },
    }
}

#[cfg(not(target_arch = "wasm32"))]
const DVM_QUOTE_WINDOW: Duration = Duration::from_secs(5);

