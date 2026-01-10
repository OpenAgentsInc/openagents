/// Claude capability as a filesystem.
pub struct ClaudeFs {
    agent_id: AgentId,
    router: Arc<RwLock<ClaudeRouter>>,
    policy: Arc<RwLock<ClaudePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
    auth_state: Arc<RwLock<TunnelAuthState>>,
    signer: Arc<dyn SigningService>,
    pool: Arc<RwLock<PoolState>>,
    proxy: Arc<RwLock<ProxyState>>,
}

impl ClaudeFs {
    pub fn new(
        agent_id: AgentId,
        router: ClaudeRouter,
        policy: ClaudePolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        Self::with_state(
            agent_id,
            router,
            policy,
            budget_policy,
            journal,
            signer,
            Arc::new(RwLock::new(Vec::new())),
            Arc::new(RwLock::new(TunnelAuthState::default())),
        )
    }

    pub fn with_state(
        agent_id: AgentId,
        router: ClaudeRouter,
        policy: ClaudePolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        signer: Arc<dyn SigningService>,
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
    ) -> Self {
        Self {
            agent_id,
            router: Arc::new(RwLock::new(router)),
            policy: Arc::new(RwLock::new(policy)),
            budget: Arc::new(Mutex::new(BudgetTracker::new(budget_policy))),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            journal,
            tunnels,
            auth_state,
            signer,
            pool: Arc::new(RwLock::new(PoolState::default())),
            proxy: Arc::new(RwLock::new(ProxyState::default())),
        }
    }

    pub fn tunnels(&self) -> Arc<RwLock<Vec<TunnelEndpoint>>> {
        self.tunnels.clone()
    }

    pub fn auth_state(&self) -> Arc<RwLock<TunnelAuthState>> {
        self.auth_state.clone()
    }

    fn session_provider(&self, session_id: &str) -> FsResult<(Arc<dyn ClaudeProvider>, String)> {
        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
        let record = sessions.get(session_id).ok_or(FsError::NotFound)?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)?;
        Ok((provider, record.provider_id.clone()))
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
            }
            SessionState::Failed { .. } => {
                tracker.release(record.reservation);
            }
            _ => return Ok(()),
        }

        record.reconciled = true;
        Ok(())
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

    fn session_usage_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        self.reconcile_session(session_id, &state)?;
        let reserved_usd = self
            .sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|record| record.reservation.amount_usd)
            .unwrap_or(0);

        let (usage, cost_usd) = match state {
            SessionState::Complete(response) => (response.usage, response.cost_usd),
            SessionState::Idle {
                usage, cost_usd, ..
            } => (usage, cost_usd),
            _ => (None, 0),
        };
        let json = serde_json::json!({
            "reserved_usd": reserved_usd,
            "cost_usd": cost_usd,
            "usage": usage,
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn session_response_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        self.reconcile_session(session_id, &state)?;
        let reserved_usd = self
            .sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|record| record.reservation.amount_usd)
            .unwrap_or(0);

        match state {
            SessionState::Complete(response) => {
                let json = serde_json::json!({
                    "session_id": response.session_id,
                    "status": response.status,
                    "response": response.response,
                    "usage": response.usage,
                    "cost_usd": response.cost_usd,
                    "reserved_usd": reserved_usd,
                    "provider_id": response.provider_id,
                    "model": response.model,
                    "tunnel_endpoint": response.tunnel_endpoint,
                });
                serde_json::to_vec_pretty(&json).map_err(|err| FsError::Other(err.to_string()))
            }
            SessionState::Idle {
                response,
                usage,
                cost_usd,
                ..
            } => {
                let json = serde_json::json!({
                    "session_id": session_id,
                    "status": ClaudeSessionStatus::Idle,
                    "response": response,
                    "usage": usage,
                    "cost_usd": cost_usd,
                    "reserved_usd": reserved_usd,
                });
                serde_json::to_vec_pretty(&json).map_err(|err| FsError::Other(err.to_string()))
            }
            SessionState::Failed { error, .. } => Err(FsError::Other(error)),
            _ => Err(FsError::Other("not ready".to_string())),
        }
    }

    fn session_context_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        let response = match state {
            SessionState::Complete(response) => response.response,
            SessionState::Idle { response, .. } => response,
            _ => None,
        };
        let json = serde_json::json!({
            "session_id": session_id,
            "latest_response": response,
        });
        serde_json::to_vec_pretty(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn session_status_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        self.reconcile_session(session_id, &state)?;
        let json = serde_json::json!({
            "status": state.status(),
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn tool_log_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let entries = provider.tool_log(session_id).unwrap_or_default();
        serde_json::to_vec_pretty(&entries).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pending_tool_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let pending = provider.pending_tool(session_id);
        serde_json::to_vec_pretty(&pending).map_err(|err| FsError::Other(err.to_string()))
    }

    fn auth_status_json(&self) -> FsResult<Vec<u8>> {
        let now = Timestamp::now();
        let tunnels = self.tunnels.read().unwrap_or_else(|e| e.into_inner());
        let mut auth = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
        let mut statuses = Vec::new();
        for tunnel in tunnels.iter() {
            let challenge = auth.challenges.get(&tunnel.id).cloned();
            let response = auth.responses.get(&tunnel.id).cloned();
            let (authorized, pubkey) = match tunnel.auth {
                TunnelAuth::None => (true, None),
                TunnelAuth::Nostr { .. } => {
                    if let (Some(challenge), Some(response)) = (&challenge, response) {
                        if challenge.expires_at.as_millis() <= now.as_millis()
                            || response.challenge != challenge.challenge
                        {
                            auth.responses.remove(&tunnel.id);
                            (false, None)
                        } else {
                            (true, Some(response.pubkey))
                        }
                    } else {
                        (false, None)
                    }
                }
                TunnelAuth::Psk { .. } => (false, None),
            };
            statuses.push(TunnelAuthStatus {
                tunnel_id: tunnel.id.clone(),
                auth_type: tunnel.auth.type_name(),
                authorized,
                pubkey,
                challenge_expires_at: challenge.map(|c| c.expires_at),
            });
        }
        serde_json::to_vec_pretty(&statuses).map_err(|err| FsError::Other(err.to_string()))
    }

    fn tunnel_endpoints_json(&self) -> FsResult<Vec<u8>> {
        let guard = self.tunnels.read().unwrap_or_else(|e| e.into_inner());
        let summary: Vec<TunnelSummary> = guard
            .iter()
            .map(|t| TunnelSummary {
                id: t.id.clone(),
                url: t.url.clone(),
                auth_type: t.auth.type_name(),
            })
            .collect();
        serde_json::to_vec_pretty(&summary).map_err(|err| FsError::Other(err.to_string()))
    }

    fn provider_health_json(&self, provider_id: &str) -> FsResult<Vec<u8>> {
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let info = router
            .list_providers()
            .into_iter()
            .find(|p| p.id == provider_id)
            .ok_or(FsError::NotFound)?;
        let json = serde_json::json!({ "status": info.status });
        serde_json::to_vec_pretty(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pool_config_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.config).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pool_status_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        let status = PoolStatus {
            total_workers: state.workers.len() as u32,
            idle_workers: state
                .workers
                .values()
                .filter(|w| w.status == WorkerStatus::Idle)
                .count() as u32,
            unhealthy_workers: state
                .workers
                .values()
                .filter(|w| w.status == WorkerStatus::Unhealthy)
                .count() as u32,
        };
        serde_json::to_vec_pretty(&status).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pool_metrics_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.metrics).map_err(|err| FsError::Other(err.to_string()))
    }

    fn proxy_status_json(&self) -> FsResult<Vec<u8>> {
        let state = self.proxy.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.status).map_err(|err| FsError::Other(err.to_string()))
    }

    fn proxy_metrics_json(&self) -> FsResult<Vec<u8>> {
        let state = self.proxy.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.metrics).map_err(|err| FsError::Other(err.to_string()))
    }

    fn proxy_allowlist_json(&self) -> FsResult<Vec<u8>> {
        let state = self.proxy.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.allowlist).map_err(|err| FsError::Other(err.to_string()))
    }

    fn workers_list_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        let workers: Vec<_> = state.workers.values().cloned().collect();
        serde_json::to_vec_pretty(&workers).map_err(|err| FsError::Other(err.to_string()))
    }

    fn worker_field_json(&self, worker_id: &str, field: WorkerField) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        let worker = state.workers.get(worker_id).ok_or(FsError::NotFound)?;
        let json = match field {
            WorkerField::Status => serde_json::to_vec_pretty(&worker.status),
            WorkerField::Isolation => serde_json::to_vec_pretty(&worker.isolation),
            WorkerField::Sessions => serde_json::to_vec_pretty(&worker.sessions),
            WorkerField::Metrics => serde_json::to_vec_pretty(&worker.metrics),
        };
        json.map_err(|err| FsError::Other(err.to_string()))
    }
}

impl FileService for ClaudeFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            ["new"] if flags.write => Ok(Box::new(ClaudeNewHandle::new(
                self.agent_id.clone(),
                self.router.clone(),
                self.policy.clone(),
                self.budget.clone(),
                self.sessions.clone(),
                self.journal.clone(),
            ))),
            ["policy"] => {
                if flags.write {
                    Ok(Box::new(ClaudePolicyWriteHandle::new(self.policy.clone())))
                } else {
                    let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                    let json = serde_json::to_vec_pretty(&*policy)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    Ok(Box::new(BytesHandle::new(json)))
                }
            }
            ["usage"] => Ok(Box::new(BytesHandle::new(self.usage_json()?))),
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
            ["providers", id, "models"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info.models)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "health"] => {
                Ok(Box::new(BytesHandle::new(self.provider_health_json(id)?)))
            }
            ["providers", "tunnel", "endpoints"] => {
                Ok(Box::new(BytesHandle::new(self.tunnel_endpoints_json()?)))
            }
            ["auth", "tunnels"] => {
                if flags.write {
                    Ok(Box::new(AuthTunnelsWriteHandle::new(
                        self.tunnels.clone(),
                        self.auth_state.clone(),
                    )))
                } else {
                    Ok(Box::new(AuthTunnelsReadHandle::new(self.tunnels.clone())))
                }
            }
            ["auth", "challenge"] => {
                if flags.write {
                    Ok(Box::new(AuthChallengeWriteHandle::new(
                        self.tunnels.clone(),
                        self.auth_state.clone(),
                        self.signer.clone(),
                    )))
                } else {
                    Ok(Box::new(AuthChallengeReadHandle::new(
                        self.tunnels.clone(),
                        self.auth_state.clone(),
                    )))
                }
            }
            ["auth", "status"] => Ok(Box::new(BytesHandle::new(self.auth_status_json()?))),
            ["sessions", session_id, "status"] => Ok(Box::new(BytesHandle::new(
                self.session_status_json(session_id)?,
            ))),
            ["sessions", session_id, "prompt"] if flags.write => Ok(Box::new(PromptHandle::new(
                self.router.clone(),
                session_id.to_string(),
            ))),
            ["sessions", session_id, "response"] => Ok(Box::new(BytesHandle::new(
                self.session_response_json(session_id)?,
            ))),
            ["sessions", session_id, "context"] => Ok(Box::new(BytesHandle::new(
                self.session_context_json(session_id)?,
            ))),
            ["sessions", session_id, "usage"] => Ok(Box::new(BytesHandle::new(
                self.session_usage_json(session_id)?,
            ))),
            ["sessions", session_id, "tools", "log"] => {
                Ok(Box::new(BytesHandle::new(self.tool_log_json(session_id)?)))
            }
            ["sessions", session_id, "tools", "pending"] => Ok(Box::new(BytesHandle::new(
                self.pending_tool_json(session_id)?,
            ))),
            ["sessions", session_id, "tools", "approve"] if flags.write => Ok(Box::new(
                ToolApprovalHandle::new(self.router.clone(), session_id.to_string()),
            )),
            ["sessions", session_id, "fork"] if flags.write => Ok(Box::new(ForkHandle::new(
                self.router.clone(),
                session_id.to_string(),
            ))),
            ["sessions", session_id, "ctl"] if flags.write => Ok(Box::new(SessionCtlHandle::new(
                self.router.clone(),
                session_id.to_string(),
            ))),
            ["pool", "config"] => {
                if flags.write {
                    Ok(Box::new(PoolConfigWriteHandle::new(self.pool.clone())))
                } else {
                    Ok(Box::new(BytesHandle::new(self.pool_config_json()?)))
                }
            }
            ["pool", "status"] => Ok(Box::new(BytesHandle::new(self.pool_status_json()?))),
            ["pool", "metrics"] => Ok(Box::new(BytesHandle::new(self.pool_metrics_json()?))),
            ["proxy", "status"] => Ok(Box::new(BytesHandle::new(self.proxy_status_json()?))),
            ["proxy", "metrics"] => Ok(Box::new(BytesHandle::new(self.proxy_metrics_json()?))),
            ["proxy", "allowlist"] => {
                if flags.write {
                    Ok(Box::new(ProxyAllowlistWriteHandle::new(self.proxy.clone())))
                } else {
                    Ok(Box::new(BytesHandle::new(self.proxy_allowlist_json()?)))
                }
            }
            ["workers"] => Ok(Box::new(BytesHandle::new(self.workers_list_json()?))),
            ["workers", worker_id, "status"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Status)?,
            ))),
            ["workers", worker_id, "isolation"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Isolation)?,
            ))),
            ["workers", worker_id, "sessions"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Sessions)?,
            ))),
            ["workers", worker_id, "metrics"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Metrics)?,
            ))),
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
                DirEntry::dir("workers"),
                DirEntry::dir("pool"),
                DirEntry::dir("proxy"),
            ]),
            "providers" => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                Ok(router
                    .list_providers()
                    .iter()
                    .map(|p| DirEntry::dir(&p.id))
                    .collect())
            }
            "auth" => Ok(vec![
                DirEntry::file("tunnels", 0),
                DirEntry::file("challenge", 0),
                DirEntry::file("status", 0),
            ]),
            "sessions" => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                Ok(sessions.keys().map(|id| DirEntry::dir(id)).collect())
            }
            "workers" => {
                let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                Ok(pool.workers.keys().map(|id| DirEntry::dir(id)).collect())
            }
            "pool" => Ok(vec![
                DirEntry::file("config", 0),
                DirEntry::file("status", 0),
                DirEntry::file("metrics", 0),
            ]),
            "proxy" => Ok(vec![
                DirEntry::file("status", 0),
                DirEntry::file("allowlist", 0),
                DirEntry::file("metrics", 0),
            ]),
            _ => {
                let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
                match parts.as_slice() {
                    ["providers", id] => {
                        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                        if router.list_providers().iter().any(|p| p.id == *id) {
                            let mut entries = vec![
                                DirEntry::file("info", 0),
                                DirEntry::file("models", 0),
                                DirEntry::file("health", 0),
                            ];
                            if *id == "tunnel" {
                                entries.push(DirEntry::file("endpoints", 0));
                            }
                            Ok(entries)
                        } else {
                            Ok(vec![])
                        }
                    }
                    ["sessions", session_id] => {
                        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                        if sessions.contains_key(*session_id) {
                            Ok(vec![
                                DirEntry::file("status", 0),
                                DirEntry::file("prompt", 0),
                                DirEntry::file("response", 0),
                                DirEntry::file("context", 0),
                                DirEntry::file("output", 0),
                                DirEntry::file("usage", 0),
                                DirEntry::dir("tools"),
                                DirEntry::file("fork", 0),
                                DirEntry::file("ctl", 0),
                            ])
                        } else {
                            Ok(vec![])
                        }
                    }
                    ["sessions", session_id, "tools"] => {
                        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                        if sessions.contains_key(*session_id) {
                            Ok(vec![
                                DirEntry::file("log", 0),
                                DirEntry::file("pending", 0),
                                DirEntry::file("approve", 0),
                            ])
                        } else {
                            Ok(vec![])
                        }
                    }
                    ["workers", worker_id] => {
                        let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                        if pool.workers.contains_key(*worker_id) {
                            Ok(vec![
                                DirEntry::file("status", 0),
                                DirEntry::file("isolation", 0),
                                DirEntry::file("sessions", 0),
                                DirEntry::file("metrics", 0),
                            ])
                        } else {
                            Ok(vec![])
                        }
                    }
                    _ => Ok(vec![]),
                }
            }
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            [] => Ok(Stat::dir()),
            ["providers"] | ["sessions"] | ["auth"] | ["workers"] | ["pool"] | ["proxy"] => {
                Ok(Stat::dir())
            }
            ["new"] | ["policy"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions::read_write(),
            }),
            ["usage"] => Ok(Stat::file(0)),
            ["providers", id] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["providers", id, "info"]
            | ["providers", id, "models"]
            | ["providers", id, "health"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["providers", "tunnel", "endpoints"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == "tunnel") {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["auth", "tunnels"] | ["auth", "challenge"] | ["auth", "status"] => Ok(Stat::file(0)),
            ["sessions", session_id] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "tools"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, _] | ["sessions", session_id, "tools", _] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["pool", "config"] | ["pool", "status"] | ["pool", "metrics"] => Ok(Stat::file(0)),
            ["proxy", "status"] | ["proxy", "metrics"] | ["proxy", "allowlist"] => {
                Ok(Stat::file(0))
            }
            ["workers", worker_id] => {
                let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                if pool.workers.contains_key(*worker_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["workers", worker_id, _] => {
                let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                if pool.workers.contains_key(*worker_id) {
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
            let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            let record = sessions.get(*session_id).ok_or(FsError::NotFound)?;
            return Ok(Some(Box::new(OutputWatchHandle::new(
                session_id.to_string(),
                record.provider_id.clone(),
                self.router.clone(),
                self.sessions.clone(),
                self.budget.clone(),
            ))));
        }
        Ok(None)
    }

    fn name(&self) -> &str {
        "claude"
    }
}

struct ClaudeNewHandle {
    agent_id: AgentId,
    router: Arc<RwLock<ClaudeRouter>>,
    policy: Arc<RwLock<ClaudePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ClaudeNewHandle {
    fn new(
        agent_id: AgentId,
        router: Arc<RwLock<ClaudeRouter>>,
        policy: Arc<RwLock<ClaudePolicy>>,
        budget: Arc<Mutex<BudgetTracker>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router,
            policy,
            budget,
            sessions,
            journal,
            request_buf: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit_request(&mut self) -> FsResult<()> {
        let mut request: ClaudeRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|err| FsError::Other(err.to_string()))?;

        let policy = self
            .policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(FsError::Other(ClaudeError::IdempotencyRequired.to_string()));
        }

        let resolved_autonomy = request
            .autonomy
            .clone()
            .unwrap_or_else(|| policy.default_autonomy.clone());
        request.autonomy = Some(resolved_autonomy.clone());

        if let Some(limit) = policy.max_context_tokens {
            if let Some(requested) = request.max_context_tokens {
                request.max_context_tokens = Some(requested.min(limit));
            } else {
                request.max_context_tokens = Some(limit);
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
                return Err(FsError::Other(ClaudeError::MaxCostRequired.to_string()));
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
        request.max_cost_usd = Some(max_cost_usd);

        if !policy.allowed_models.is_empty()
            && !policy
                .allowed_models
                .iter()
                .any(|pat| matches_pattern(pat, &request.model))
        {
            return Err(FsError::Other(
                ClaudeError::InvalidRequest("model not allowed".to_string()).to_string(),
            ));
        }

        if policy
            .blocked_models
            .iter()
            .any(|pat| matches_pattern(pat, &request.model))
        {
            return Err(FsError::Other(
                ClaudeError::InvalidRequest("model blocked".to_string()).to_string(),
            ));
        }

        if let Some(tunnel) = request.tunnel_endpoint.as_ref() {
            if !policy.allowed_tunnels.is_empty()
                && !policy.allowed_tunnels.iter().any(|t| t == tunnel)
            {
                return Err(FsError::Other(
                    ClaudeError::InvalidRequest("tunnel not allowed".to_string()).to_string(),
                ));
            }
        }

        let active_sessions = {
            let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            let router = self.router.read().unwrap_or_else(|e| e.into_inner());
            sessions
                .iter()
                .filter(|(session_id, record)| {
                    let provider = match router.provider_by_id(&record.provider_id) {
                        Some(provider) => provider,
                        None => return false,
                    };
                    let state = match provider.get_session(session_id) {
                        Some(state) => state,
                        None => return false,
                    };
                    !matches!(
                        state,
                        SessionState::Complete(_) | SessionState::Failed { .. }
                    )
                })
                .count()
        };
        if active_sessions as u32 >= policy.max_concurrent {
            return Err(FsError::Other(
                ClaudeError::InvalidRequest("max concurrent sessions exceeded".to_string())
                    .to_string(),
            ));
        }

        if !policy.allowed_tools.is_empty() {
            for tool in &request.tools {
                if !policy.allowed_tools.iter().any(|t| t == &tool.name) {
                    return Err(FsError::Other(
                        ClaudeError::InvalidRequest(format!("tool not allowed: {}", tool.name))
                            .to_string(),
                    ));
                }
            }
        }

        for tool in &request.tools {
            if policy.blocked_tools.iter().any(|t| t == &tool.name) {
                return Err(FsError::Other(
                    ClaudeError::InvalidRequest(format!("tool blocked: {}", tool.name)).to_string(),
                ));
            }
        }

        let tool_policy = ToolPolicy {
            allowed: if !policy.allowed_tools.is_empty() {
                policy.allowed_tools.clone()
            } else {
                request.tools.iter().map(|t| t.name.clone()).collect()
            },
            blocked: policy.blocked_tools.clone(),
            approval_required: policy.approval_required_tools.clone(),
            autonomy: resolved_autonomy,
        };

        request = request.with_internal(ClaudeRequestInternal {
            tool_policy,
            fork: false,
            resume_backend_id: None,
            #[cfg(not(target_arch = "wasm32"))]
            container: None,
            #[cfg(not(target_arch = "wasm32"))]
            executable: None,
        });

        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = if let Some(resume_id) = request.resume_session_id.as_ref() {
            let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            let record = sessions
                .get(resume_id)
                .ok_or_else(|| FsError::Other("resume session not found".to_string()))?;
            router
                .provider_by_id(&record.provider_id)
                .ok_or(FsError::NotFound)?
        } else {
            router
                .select(&request, &policy)
                .map_err(|err| FsError::Other(err.to_string()))?
        };
        let provider_id = provider.id().to_string();

        #[cfg(not(target_arch = "wasm32"))]
        if matches!(policy.isolation_mode, IsolationMode::Container)
            && matches!(provider_id.as_str(), "local" | "cloud")
        {
            let config =
                resolve_container_config(&policy).map_err(|err| FsError::Other(err.to_string()))?;
            request.internal.container = Some(config);
        }
        #[cfg(target_arch = "wasm32")]
        if matches!(policy.isolation_mode, IsolationMode::Container)
            && matches!(provider_id.as_str(), "local" | "cloud")
        {
            return Err(FsError::Other(
                "container isolation not supported on wasm".to_string(),
            ));
        }

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
                            });
                    }
                }
                self.response = Some(cached);
                return Ok(());
            }
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

        let session_id = match provider.create_session(request.clone()) {
            Ok(session_id) => session_id,
            Err(err) => {
                let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                tracker.release(reservation);
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
                },
            );

        let response_json = serde_json::json!({
            "session_id": session_id,
            "status": "creating",
            "status_path": format!("/claude/sessions/{}/status", session_id),
            "output_path": format!("/claude/sessions/{}/output", session_id),
            "response_path": format!("/claude/sessions/{}/response", session_id),
            "prompt_path": format!("/claude/sessions/{}/prompt", session_id),
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

impl FileHandle for ClaudeNewHandle {
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

struct PromptHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
}

impl PromptHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
        }
    }

    fn send_prompt(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let prompt = String::from_utf8(self.buffer.clone())
            .map_err(|err| FsError::Other(err.to_string()))?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        provider
            .send_prompt(&self.session_id, &prompt)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }
}

impl FileHandle for PromptHandle {
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
        self.send_prompt()
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ToolApprovalHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
}

impl ToolApprovalHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
        }
    }

    fn submit(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        let approved = value
            .get("approved")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| FsError::Other("missing approved field".to_string()))?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        provider
            .approve_tool(&self.session_id, approved)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }
}

impl FileHandle for ToolApprovalHandle {
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
        self.submit()
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ForkHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ForkHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn fork(&mut self) -> FsResult<()> {
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        let new_id = provider
            .fork_session(&self.session_id)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let response_json = serde_json::json!({ "session_id": new_id });
        let response_bytes =
            serde_json::to_vec(&response_json).map_err(|err| FsError::Other(err.to_string()))?;
        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ForkHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.fork()?;
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
        if self.response.is_none() {
            self.fork()?;
        }
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct SessionCtlHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
}

impl SessionCtlHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
        }
    }

    fn apply(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let command = String::from_utf8(self.buffer.clone())
            .map_err(|err| FsError::Other(err.to_string()))?;
        let command = command.trim();
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        match command {
            "stop" => provider
                .stop(&self.session_id)
                .map_err(|err| FsError::Other(err.to_string()))?,
            "pause" => provider
                .pause(&self.session_id)
                .map_err(|err| FsError::Other(err.to_string()))?,
            "resume" => provider
                .resume(&self.session_id)
                .map_err(|err| FsError::Other(err.to_string()))?,
            _ => return Err(FsError::Other("unknown command".to_string())),
        }
        self.buffer.clear();
        Ok(())
    }
}

impl FileHandle for SessionCtlHandle {
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
        self.apply()
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ClaudePolicyWriteHandle {
    policy: Arc<RwLock<ClaudePolicy>>,
    buffer: Vec<u8>,
}

impl ClaudePolicyWriteHandle {
    fn new(policy: Arc<RwLock<ClaudePolicy>>) -> Self {
        Self {
            policy,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for ClaudePolicyWriteHandle {
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
        let policy: ClaudePolicy =
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

struct AuthTunnelsReadHandle {
    position: usize,
    payload: Vec<u8>,
}

impl AuthTunnelsReadHandle {
    fn new(tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>) -> Self {
        let guard = tunnels.read().unwrap_or_else(|e| e.into_inner());
        let summary: Vec<TunnelSummary> = guard
            .iter()
            .map(|t| TunnelSummary {
                id: t.id.clone(),
                url: t.url.clone(),
                auth_type: t.auth.type_name(),
            })
            .collect();
        let payload = serde_json::to_vec_pretty(&summary).unwrap_or_default();
        Self {
            position: 0,
            payload,
        }
    }
}

impl FileHandle for AuthTunnelsReadHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.position >= self.payload.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), self.payload.len() - self.position);
        buf[..len].copy_from_slice(&self.payload[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.payload.len() as i64 + offset,
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
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct AuthTunnelsWriteHandle {
    tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
    auth_state: Arc<RwLock<TunnelAuthState>>,
    buffer: Vec<u8>,
}

impl AuthTunnelsWriteHandle {
    fn new(
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
    ) -> Self {
        Self {
            tunnels,
            auth_state,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for AuthTunnelsWriteHandle {
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
        let endpoints: Vec<TunnelEndpoint> =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        let mut guard = self.tunnels.write().unwrap_or_else(|e| e.into_inner());
        *guard = endpoints;
        let mut auth_state = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
        auth_state
            .responses
            .retain(|id, _| guard.iter().any(|t| &t.id == id));
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct AuthChallengeReadHandle {
    position: usize,
    payload: Vec<u8>,
}

impl AuthChallengeReadHandle {
    fn new(
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
    ) -> Self {
        let now = Timestamp::now();
        let guard = tunnels.read().unwrap_or_else(|e| e.into_inner());
        let mut auth = auth_state.write().unwrap_or_else(|e| e.into_inner());
        let mut challenges = Vec::new();

        for endpoint in guard.iter() {
            let mut expired = false;
            let challenge_snapshot = {
                let challenge = auth
                    .challenges
                    .entry(endpoint.id.clone())
                    .or_insert_with(|| TunnelAuthChallenge {
                        challenge: uuid::Uuid::new_v4().to_string(),
                        expires_at: Timestamp::from_millis(
                            now.as_millis() + AUTH_CHALLENGE_TTL.as_millis() as u64,
                        ),
                        tunnel_id: endpoint.id.clone(),
                    });
                if challenge.expires_at.as_millis() <= now.as_millis() {
                    *challenge = TunnelAuthChallenge {
                        challenge: uuid::Uuid::new_v4().to_string(),
                        expires_at: Timestamp::from_millis(
                            now.as_millis() + AUTH_CHALLENGE_TTL.as_millis() as u64,
                        ),
                        tunnel_id: endpoint.id.clone(),
                    };
                    expired = true;
                }
                challenge.clone()
            };
            if expired {
                auth.responses.remove(&endpoint.id);
            }
            challenges.push(challenge_snapshot);
        }

        let payload = serde_json::to_vec_pretty(&challenges).unwrap_or_default();
        Self {
            position: 0,
            payload,
        }
    }
}

impl FileHandle for AuthChallengeReadHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.position >= self.payload.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), self.payload.len() - self.position);
        buf[..len].copy_from_slice(&self.payload[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.payload.len() as i64 + offset,
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
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct AuthChallengeWriteHandle {
    tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
    auth_state: Arc<RwLock<TunnelAuthState>>,
    signer: Arc<dyn SigningService>,
    buffer: Vec<u8>,
}

impl AuthChallengeWriteHandle {
    fn new(
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        Self {
            tunnels,
            auth_state,
            signer,
            buffer: Vec::new(),
        }
    }

    fn verify_response(&self, response: &TunnelAuthResponse) -> FsResult<()> {
        let tunnels = self.tunnels.read().unwrap_or_else(|e| e.into_inner());
        let endpoint = tunnels
            .iter()
            .find(|t| t.id == response.tunnel_id)
            .ok_or_else(|| FsError::Other("unknown tunnel".to_string()))?;
        let mut auth_state = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
        let challenge = auth_state
            .challenges
            .get(&response.tunnel_id)
            .ok_or_else(|| FsError::Other("no challenge".to_string()))?;
        if challenge.challenge != response.challenge {
            return Err(FsError::Other("challenge mismatch".to_string()));
        }
        if challenge.expires_at.as_millis() <= Timestamp::now().as_millis() {
            return Err(FsError::Other("challenge expired".to_string()));
        }

        let pubkey = parse_pubkey(&response.pubkey)?;
        let signature = parse_signature(&response.signature)?;

        if !endpoint.allowed_agents.is_empty() {
            let allowed = endpoint.allowed_agents.iter().any(|agent| {
                if let Ok(candidate) = parse_pubkey(agent) {
                    candidate.as_bytes() == pubkey.as_bytes()
                } else {
                    agent.eq_ignore_ascii_case(&response.pubkey)
                }
            });
            if !allowed {
                return Err(FsError::Other("agent not allowed".to_string()));
            }
        }

        if !self
            .signer
            .verify(&pubkey, response.challenge.as_bytes(), &signature)
        {
            return Err(FsError::Other("invalid signature".to_string()));
        }

        auth_state
            .responses
            .insert(response.tunnel_id.clone(), response.clone());

        Ok(())
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
        let response: TunnelAuthResponse =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        self.verify_response(&response)?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct PoolConfigWriteHandle {
    pool: Arc<RwLock<PoolState>>,
    buffer: Vec<u8>,
}

impl PoolConfigWriteHandle {
    fn new(pool: Arc<RwLock<PoolState>>) -> Self {
        Self {
            pool,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for PoolConfigWriteHandle {
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
        let config: PoolConfig =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        let mut state = self.pool.write().unwrap_or_else(|e| e.into_inner());
        state.config = config;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ProxyAllowlistWriteHandle {
    proxy: Arc<RwLock<ProxyState>>,
    buffer: Vec<u8>,
}

impl ProxyAllowlistWriteHandle {
    fn new(proxy: Arc<RwLock<ProxyState>>) -> Self {
        Self {
            proxy,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for ProxyAllowlistWriteHandle {
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
        let allowlist: Vec<String> =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        let mut proxy = self.proxy.write().unwrap_or_else(|e| e.into_inner());
        proxy.allowlist = allowlist;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct OutputWatchHandle {
    session_id: String,
    provider_id: String,
    router: Arc<RwLock<ClaudeRouter>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    budget: Arc<Mutex<BudgetTracker>>,
}

impl OutputWatchHandle {
    fn new(
        session_id: String,
        provider_id: String,
        router: Arc<RwLock<ClaudeRouter>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        budget: Arc<Mutex<BudgetTracker>>,
    ) -> Self {
        Self {
            session_id,
            provider_id,
            router,
            sessions,
            budget,
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
            }
            SessionState::Failed { .. } => {
                tracker.release(record.reservation);
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }
}

impl WatchHandle for OutputWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            let router = self.router.read().unwrap_or_else(|e| e.into_inner());
            let provider = router
                .provider_by_id(&self.provider_id)
                .ok_or(FsError::NotFound)?;
            match provider.poll_output(&self.session_id) {
                Ok(Some(chunk)) => {
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = provider.get_session(&self.session_id) {
                        if matches!(
                            state,
                            SessionState::Complete(_) | SessionState::Failed { .. }
                        ) {
                            self.reconcile(&state)?;
                            return Ok(None);
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }

            if !wait_for_stream(deadline)? {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

