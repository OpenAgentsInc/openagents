/// Job record tracked by ComputeFs for budgeting.
#[derive(Clone)]
struct JobRecord {
    provider_id: String,
    reservation: BudgetReservation,
    reconciled: bool,
}

/// Compute capability as a filesystem.
pub struct ComputeFs {
    agent_id: AgentId,
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    journal: Arc<dyn IdempotencyJournal>,
    jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
}

impl ComputeFs {
    /// Create a new compute filesystem.
    pub fn new(
        agent_id: AgentId,
        router: ComputeRouter,
        policy: ComputePolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router: Arc::new(RwLock::new(router)),
            policy: Arc::new(RwLock::new(policy)),
            budget: Arc::new(Mutex::new(BudgetTracker::new(budget_policy))),
            journal,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn job_provider(&self, job_id: &str) -> FsResult<(Arc<dyn ComputeProvider>, String)> {
        let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
        let record = jobs.get(job_id).ok_or(FsError::NotFound)?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)?;
        Ok((provider, record.provider_id.clone()))
    }

    fn reconcile_job(&self, job_id: &str, state: &JobState) -> FsResult<()> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let record = match jobs.get_mut(job_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }

        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            JobState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
            }
            JobState::Failed { .. } => {
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
}

impl FileService for ComputeFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            ["new"] if flags.write => Ok(Box::new(ComputeNewHandle::new(
                self.agent_id.clone(),
                self.router.clone(),
                self.policy.clone(),
                self.budget.clone(),
                self.jobs.clone(),
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
            ["jobs", job_id, "status"] => {
                let (provider, _) = self.job_provider(job_id)?;
                let state = provider.get_job(job_id).ok_or(FsError::NotFound)?;
                self.reconcile_job(job_id, &state)?;
                let status = match state {
                    JobState::Pending { .. } => "pending",
                    JobState::Running { .. } => "running",
                    JobState::Streaming { .. } => "streaming",
                    JobState::Complete(_) => "complete",
                    JobState::Failed { .. } => "failed",
                };
                let json = format!(r#"{{"status":"{}"}}"#, status);
                Ok(Box::new(BytesHandle::new(json.into_bytes())))
            }
            ["jobs", job_id, "result"] => {
                let (provider, _) = self.job_provider(job_id)?;
                let state = provider.get_job(job_id).ok_or(FsError::NotFound)?;
                self.reconcile_job(job_id, &state)?;
                match state {
                    JobState::Complete(response) => {
                        let json = serde_json::to_vec_pretty(&response)
                            .map_err(|err| FsError::Other(err.to_string()))?;
                        Ok(Box::new(BytesHandle::new(json)))
                    }
                    JobState::Failed { error, .. } => Err(FsError::Other(error)),
                    _ => Err(FsError::Other("not ready".to_string())),
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
                DirEntry::dir("jobs"),
            ]),
            "providers" => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                Ok(router
                    .list_providers()
                    .iter()
                    .map(|p| DirEntry::dir(&p.id))
                    .collect())
            }
            "jobs" => {
                let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
                Ok(jobs.keys().map(|id| DirEntry::dir(id)).collect())
            }
            _ => Ok(vec![]),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            [] => Ok(Stat::dir()),
            ["providers"] | ["jobs"] => Ok(Stat::dir()),
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
            ["providers", id, "info"] | ["providers", id, "models"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["jobs", job_id] => {
                let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
                if jobs.contains_key(*job_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["jobs", job_id, "status"]
            | ["jobs", job_id, "result"]
            | ["jobs", job_id, "stream"] => {
                let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
                if jobs.contains_key(*job_id) {
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
        if let ["jobs", job_id, "stream"] = parts.as_slice() {
            let jobs = self.jobs.read().unwrap_or_else(|e| e.into_inner());
            let record = jobs.get(*job_id).ok_or(FsError::NotFound)?;
            return Ok(Some(Box::new(StreamWatchHandle::new(
                job_id.to_string(),
                record.provider_id.clone(),
                self.router.clone(),
                self.jobs.clone(),
                self.budget.clone(),
            ))));
        }
        Ok(None)
    }

    fn name(&self) -> &str {
        "compute"
    }
}

struct ComputeNewHandle {
    agent_id: AgentId,
    router: Arc<RwLock<ComputeRouter>>,
    policy: Arc<RwLock<ComputePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ComputeNewHandle {
    fn new(
        agent_id: AgentId,
        router: Arc<RwLock<ComputeRouter>>,
        policy: Arc<RwLock<ComputePolicy>>,
        budget: Arc<Mutex<BudgetTracker>>,
        jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router,
            policy,
            budget,
            jobs,
            journal,
            request_buf: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit_request(&mut self) -> FsResult<()> {
        let mut request: ComputeRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|err| FsError::Other(err.to_string()))?;

        let policy = self
            .policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(FsError::Other(
                ComputeError::IdempotencyRequired.to_string(),
            ));
        }

        if request.timeout_ms.is_none() {
            request.timeout_ms = Some(policy.default_timeout_ms);
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
                return Err(FsError::Other(ComputeError::MaxCostRequired.to_string()));
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
                    if let Some(job_id) = value.get("job_id").and_then(|v| v.as_str()) {
                        self.jobs
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry(job_id.to_string())
                            .or_insert(JobRecord {
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

        let job_id = match provider.submit(request.clone()) {
            Ok(job_id) => job_id,
            Err(err) => {
                let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                tracker.release(reservation);
                return Err(FsError::Other(err.to_string()));
            }
        };

        self.jobs.write().unwrap_or_else(|e| e.into_inner()).insert(
            job_id.clone(),
            JobRecord {
                provider_id,
                reservation,
                reconciled: false,
            },
        );

        let response_json = serde_json::json!({
            "job_id": job_id,
            "status": "pending",
            "status_path": format!("/compute/jobs/{}/status", job_id),
            "stream_path": format!("/compute/jobs/{}/stream", job_id),
            "result_path": format!("/compute/jobs/{}/result", job_id),
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

impl FileHandle for ComputeNewHandle {
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
    policy: Arc<RwLock<ComputePolicy>>,
    buffer: Vec<u8>,
}

impl PolicyWriteHandle {
    fn new(policy: Arc<RwLock<ComputePolicy>>) -> Self {
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
        let policy: ComputePolicy =
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

struct StreamWatchHandle {
    job_id: String,
    provider_id: String,
    router: Arc<RwLock<ComputeRouter>>,
    jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
    budget: Arc<Mutex<BudgetTracker>>,
    emitted_chunk: bool,
}

impl StreamWatchHandle {
    fn new(
        job_id: String,
        provider_id: String,
        router: Arc<RwLock<ComputeRouter>>,
        jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
        budget: Arc<Mutex<BudgetTracker>>,
    ) -> Self {
        Self {
            job_id,
            provider_id,
            router,
            jobs,
            budget,
            emitted_chunk: false,
        }
    }

    fn reconcile(&self, state: &JobState) -> FsResult<()> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let record = match jobs.get_mut(&self.job_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }
        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            JobState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
            }
            JobState::Failed { .. } => {
                tracker.release(record.reservation);
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }
}

impl WatchHandle for StreamWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            let provider = {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                router
                    .provider_by_id(&self.provider_id)
                    .ok_or(FsError::NotFound)?
            };
            match provider.poll_stream(&self.job_id) {
                Ok(Some(chunk)) => {
                    self.emitted_chunk = true;
                    if let Some(state) = provider.get_job(&self.job_id) {
                        self.reconcile(&state)?;
                    }
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = provider.get_job(&self.job_id) {
                        match &state {
                            JobState::Complete(response) => {
                                self.reconcile(&state)?;
                                if !self.emitted_chunk {
                                    self.emitted_chunk = true;
                                    let chunk = ComputeChunk {
                                        job_id: self.job_id.clone(),
                                        delta: response.output.clone(),
                                        finish_reason: Some("complete".to_string()),
                                        usage: response.usage.clone(),
                                    };
                                    let payload = serde_json::to_vec(&chunk)
                                        .map_err(|err| FsError::Other(err.to_string()))?;
                                    return Ok(Some(WatchEvent::Data(payload)));
                                }
                                return Ok(None);
                            }
                            JobState::Failed { .. } => {
                                self.reconcile(&state)?;
                                return Ok(None);
                            }
                            _ => {}
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

fn wait_for_stream(deadline: Option<Instant>) -> FsResult<bool> {
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
        std::thread::sleep(Duration::from_millis(25));
        Ok(true)
    }
}

