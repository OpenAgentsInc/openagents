/// NIP-90 DVM container provider.
#[cfg(not(target_arch = "wasm32"))]
pub struct DvmContainerProvider {
    agent_id: AgentId,
    transport: Arc<dyn DvmTransport>,
    signer: Arc<dyn SigningService>,
    wallet: Option<Arc<dyn WalletService>>,
    fx: Arc<FxRateCache>,
    executor: AsyncExecutor,
    sessions: Arc<RwLock<HashMap<String, DvmContainerSession>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct DvmContainerQuote {
    provider_pubkey: String,
    price_sats: u64,
    price_usd: u64,
    event_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
#[allow(dead_code)]
enum DvmContainerLifecycle {
    AwaitingQuotes {
        since: Timestamp,
        timeout_at: Timestamp,
    },
    Processing {
        accepted_at: Timestamp,
        provider: String,
    },
    PendingSettlement {
        result_at: Timestamp,
        invoice: Option<String>,
    },
    Settled {
        settled_at: Timestamp,
    },
    Failed {
        error: String,
        at: Timestamp,
    },
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
#[allow(dead_code)]
struct DvmContainerSession {
    session_id: String,
    request_event_id: String,
    request: ContainerRequest,
    submitted_at: Timestamp,
    lifecycle: DvmContainerLifecycle,
    quotes: Vec<DvmContainerQuote>,
    accepted_quote: Option<DvmContainerQuote>,
    result: Option<ContainerResponse>,
    output: VecDeque<OutputChunk>,
    payment_made: bool,
    paid_amount_sats: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DvmContainerProvider {
    /// Create a new DVM container provider.
    pub fn new(
        agent_id: AgentId,
        relays: Vec<String>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ContainerError> {
        let transport = Arc::new(RelayPoolTransport::new(relays));
        Self::with_transport(
            agent_id,
            transport,
            signer,
            wallet,
            fx_source,
            fx_cache_secs,
        )
    }

    /// Create a DVM provider with custom transport (tests).
    pub(crate) fn with_transport(
        agent_id: AgentId,
        transport: Arc<dyn DvmTransport>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ContainerError> {
        let executor = AsyncExecutor::new()?;
        let runtime = executor.runtime();
        executor
            .block_on(transport.connect())
            .map_err(ContainerError::ProviderError)?;
        let wallet_fx = wallet.as_ref().map(|wallet| {
            Arc::new(WalletFxProvider::new(wallet.clone())) as Arc<dyn FxRateProvider>
        });
        let fx = Arc::new(FxRateCache::new(
            fx_source,
            fx_cache_secs,
            wallet_fx,
            runtime,
        ));
        Ok(Self {
            agent_id,
            transport,
            signer,
            wallet,
            fx,
            executor,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn build_job_request(&self, request: &ContainerRequest) -> Result<JobRequest, ContainerError> {
        let repo = request
            .repo
            .as_ref()
            .ok_or_else(|| ContainerError::InvalidRequest("repo required for dvm".to_string()))?;

        let mut sandbox = SandboxRunRequest::new(repo.url.clone(), repo.git_ref.clone());
        for command in &request.commands {
            sandbox = sandbox.add_command(command.clone());
        }

        let workdir = join_workdir(&repo.subdir, &request.workdir);
        if let Some(workdir) = workdir {
            sandbox = sandbox.with_workdir(workdir);
        }

        for (key, value) in &request.env {
            sandbox = sandbox.add_env(key.clone(), value.clone());
        }

        let limits = SandboxResourceLimits {
            max_time_secs: request.limits.max_time_secs,
            max_memory_mb: request.limits.max_memory_mb,
            max_disk_mb: request.limits.max_disk_mb,
            max_cpu_cores: request.limits.max_cpu_cores,
            allow_network: request.limits.allow_network,
        };
        sandbox = sandbox.with_limits(limits);

        let mut job = sandbox
            .to_job_request()
            .map_err(|err| ContainerError::InvalidRequest(err.to_string()))?;
        for relay in self.transport.relays() {
            job = job.add_relay(relay);
        }

        let max_cost_usd = request.max_cost_usd.unwrap_or(100_000);
        let max_cost_sats = self
            .fx
            .usd_to_sats(max_cost_usd)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let bid_msats = u128::from(max_cost_sats) * 1000;
        let bid_msats = u64::try_from(bid_msats)
            .map_err(|_| ContainerError::ProviderError("bid overflow".to_string()))?;
        job = job.with_bid(bid_msats);
        Ok(job)
    }

    fn sign_event(
        &self,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> Result<nostr::Event, ContainerError> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let pubkey = self
            .signer
            .pubkey(&self.agent_id)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let pubkey_hex = pubkey.to_hex();
        let unsigned = UnsignedEvent {
            pubkey: pubkey_hex.clone(),
            created_at,
            kind,
            tags,
            content,
        };
        let id = get_event_hash(&unsigned)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let id_bytes =
            hex::decode(&id).map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let sig = self
            .signer
            .sign(&self.agent_id, &id_bytes)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;

        Ok(nostr::Event {
            id,
            pubkey: pubkey_hex,
            created_at,
            kind,
            tags: unsigned.tags,
            content: unsigned.content,
            sig: sig.to_hex(),
        })
    }

    fn spawn_quote_manager(&self, session_id: String) {
        let sessions = self.sessions.clone();
        let transport = self.transport.clone();
        let signer = self.signer.clone();
        let agent_id = self.agent_id.clone();
        let executor = self.executor.clone();

        executor.spawn(async move {
            tokio::time::sleep(DVM_QUOTE_WINDOW).await;

            let (request_event_id, quote) = {
                let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
                let session = match guard.get_mut(&session_id) {
                    Some(session) => session,
                    None => return,
                };
                if !matches!(
                    session.lifecycle,
                    DvmContainerLifecycle::AwaitingQuotes { .. }
                ) {
                    return;
                }
                let best = match session
                    .quotes
                    .iter()
                    .min_by_key(|quote| quote.price_usd)
                    .cloned()
                {
                    Some(best) => best,
                    None => {
                        session.lifecycle = DvmContainerLifecycle::Failed {
                            error: "no quotes received".to_string(),
                            at: Timestamp::now(),
                        };
                        return;
                    }
                };
                session.accepted_quote = Some(best.clone());
                session.lifecycle = DvmContainerLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: best.provider_pubkey.clone(),
                };
                (session.request_event_id.clone(), best)
            };

            let tags = vec![
                vec!["e".to_string(), request_event_id],
                vec!["e".to_string(), quote.event_id.clone()],
                vec!["p".to_string(), quote.provider_pubkey.clone()],
                vec!["status".to_string(), "processing".to_string()],
            ];

            let created_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let pubkey = match signer.pubkey(&agent_id) {
                Ok(pubkey) => pubkey,
                Err(_) => return,
            };
            let pubkey_hex = pubkey.to_hex();
            let unsigned = UnsignedEvent {
                pubkey: pubkey_hex.clone(),
                created_at,
                kind: KIND_JOB_FEEDBACK,
                tags,
                content: String::new(),
            };
            let id = match get_event_hash(&unsigned) {
                Ok(id) => id,
                Err(_) => return,
            };
            let id_bytes = match hex::decode(&id) {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let sig = match signer.sign(&agent_id, &id_bytes) {
                Ok(sig) => sig,
                Err(_) => return,
            };
            let event = nostr::Event {
                id,
                pubkey: pubkey_hex,
                created_at,
                kind: KIND_JOB_FEEDBACK,
                tags: unsigned.tags,
                content: unsigned.content,
                sig: sig.to_hex(),
            };
            let _ = transport.publish(event).await;
        });
    }

    fn subscribe_session_events(
        &self,
        session_id: String,
        request_event_id: String,
    ) -> Result<(), ContainerError> {
        let result_kind = KIND_JOB_SANDBOX_RUN + 1000;
        let filters = vec![
            serde_json::json!({
                "kinds": [result_kind],
                "#e": [request_event_id],
            }),
            serde_json::json!({
                "kinds": [KIND_JOB_FEEDBACK],
                "#e": [request_event_id],
            }),
        ];
        let subscription_id = format!("dvm-session-{}", request_event_id);
        let mut rx = self
            .executor
            .block_on(self.transport.subscribe(&subscription_id, &filters))
            .map_err(ContainerError::ProviderError)?;

        let sessions = self.sessions.clone();
        let fx = self.fx.clone();
        let wallet = self.wallet.clone();

        self.executor.spawn(async move {
            while let Some(event) = rx.recv().await {
                if event.kind == result_kind {
                    handle_dvm_container_result(&session_id, &event, &sessions, &fx, &wallet);
                } else if event.kind == KIND_JOB_FEEDBACK {
                    if let Some(feedback) = parse_feedback_event(&event) {
                        handle_dvm_container_feedback(
                            &session_id,
                            feedback,
                            &sessions,
                            &fx,
                            &wallet,
                        );
                    }
                }
            }
        });

        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ContainerProvider for DvmContainerProvider {
    fn id(&self) -> &str {
        "dvm"
    }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: "dvm".to_string(),
            name: "NIP-90 DVM Network".to_string(),
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
                startup_ms: 10_000,
                measured: false,
            },
            limits: ContainerLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 4.0,
                max_disk_mb: 10_240,
                max_time_secs: 1800,
                network_allowed: true,
            },
            status: if self.wallet.is_none() {
                ProviderStatus::Unavailable {
                    reason: "wallet not configured".to_string(),
                }
            } else if self.transport.relays().is_empty() {
                ProviderStatus::Unavailable {
                    reason: "no relays configured".to_string(),
                }
            } else {
                ProviderStatus::Available
            },
        }
    }

    fn is_available(&self) -> bool {
        self.wallet.is_some() && !self.transport.relays().is_empty()
    }

    fn submit(&self, request: ContainerRequest) -> Result<String, ContainerError> {
        if self.wallet.is_none() {
            return Err(ContainerError::ProviderError(
                "wallet not configured".to_string(),
            ));
        }
        let job_request = self.build_job_request(&request)?;
        let event = self.sign_event(
            job_request.kind,
            job_request.to_tags(),
            job_request.content.clone(),
        )?;
        let event_id = event.id.clone();

        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ContainerError::ProviderError)?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Timestamp::now();
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                DvmContainerSession {
                    session_id: session_id.clone(),
                    request_event_id: event_id.clone(),
                    request: request.clone(),
                    submitted_at: now,
                    lifecycle: DvmContainerLifecycle::AwaitingQuotes {
                        since: now,
                        timeout_at: Timestamp::from_millis(
                            now.as_millis() + DVM_QUOTE_WINDOW.as_millis() as u64,
                        ),
                    },
                    quotes: Vec::new(),
                    accepted_quote: None,
                    result: None,
                    output: VecDeque::new(),
                    payment_made: false,
                    paid_amount_sats: None,
                },
            );

        self.subscribe_session_events(session_id.clone(), event_id)?;
        self.spawn_quote_manager(session_id.clone());
        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let guard = self.sessions.read().unwrap_or_else(|e| e.into_inner());
        let session = guard.get(session_id)?;
        Some(match &session.lifecycle {
            DvmContainerLifecycle::AwaitingQuotes { .. } => SessionState::Provisioning {
                started_at: session.submitted_at,
            },
            DvmContainerLifecycle::Processing { accepted_at, .. } => SessionState::Running {
                started_at: *accepted_at,
                commands_completed: 0,
            },
            DvmContainerLifecycle::PendingSettlement { .. } => SessionState::Running {
                started_at: session.submitted_at,
                commands_completed: 0,
            },
            DvmContainerLifecycle::Settled { .. } => session
                .result
                .clone()
                .map(SessionState::Complete)
                .unwrap_or(SessionState::Running {
                    started_at: session.submitted_at,
                    commands_completed: 0,
                }),
            DvmContainerLifecycle::Failed { error, at } => SessionState::Failed {
                error: error.clone(),
                at: *at,
            },
        })
    }

    fn submit_exec(&self, _session_id: &str, _command: &str) -> Result<String, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "interactive".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
    }

    fn poll_exec_output(&self, _exec_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "interactive".to_string(),
            provider: "dvm".to_string(),
        })
    }

    fn cancel_exec(&self, _exec_id: &str) -> Result<(), ContainerError> {
        Err(ContainerError::NotSupported {
            capability: "interactive".to_string(),
            provider: "dvm".to_string(),
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
            provider: "dvm".to_string(),
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
            provider: "dvm".to_string(),
        })
    }

    fn stop(&self, session_id: &str) -> Result<(), ContainerError> {
        let request_event_id = {
            let mut guard = self.sessions.write().unwrap_or_else(|e| e.into_inner());
            let session = guard
                .get_mut(session_id)
                .ok_or(ContainerError::SessionNotFound)?;
            session.lifecycle = DvmContainerLifecycle::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            };
            session.request_event_id.clone()
        };

        let tags = create_deletion_tags(&[request_event_id.as_str()], Some(KIND_JOB_SANDBOX_RUN));
        let event = self.sign_event(DELETION_REQUEST_KIND, tags, String::new())?;
        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ContainerError::ProviderError)?;
        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<OutputChunk>, ContainerError> {
        let mut guard = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let session = guard
            .get_mut(session_id)
            .ok_or(ContainerError::SessionNotFound)?;
        Ok(session.output.pop_front())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_container_feedback(
    session_id: &str,
    feedback: crate::dvm::DvmFeedback,
    sessions: &Arc<RwLock<HashMap<String, DvmContainerSession>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let mut payment_request = None;

    {
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        let Some(session) = guard.get_mut(session_id) else {
            return;
        };
        if matches!(
            session.lifecycle,
            DvmContainerLifecycle::Failed { .. } | DvmContainerLifecycle::Settled { .. }
        ) {
            return;
        }

        match feedback.status {
            DvmFeedbackStatus::Quote => {
                if let Some(amount_msats) = feedback.amount_msats {
                    let price_sats = msats_to_sats(amount_msats);
                    let price_usd = match fx.sats_to_usd(price_sats) {
                        Ok(price_usd) => price_usd,
                        Err(err) => {
                            session.lifecycle = DvmContainerLifecycle::Failed {
                                error: err.to_string(),
                                at: Timestamp::now(),
                            };
                            return;
                        }
                    };
                    let quote = DvmContainerQuote {
                        provider_pubkey: feedback.provider_pubkey.clone(),
                        price_sats,
                        price_usd,
                        event_id: feedback.event_id.clone(),
                    };
                    if let Some(existing) = session
                        .quotes
                        .iter_mut()
                        .find(|q| q.provider_pubkey == quote.provider_pubkey)
                    {
                        if quote.price_usd < existing.price_usd {
                            *existing = quote;
                        }
                    } else {
                        session.quotes.push(quote);
                    }
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Partial) => {
                session.output.push_back(OutputChunk {
                    session_id: session_id.to_string(),
                    exec_id: None,
                    stream: OutputStream::Stdout,
                    data: feedback.content,
                });
            }
            DvmFeedbackStatus::Job(JobStatus::Processing) => {
                session.lifecycle = DvmContainerLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: feedback.provider_pubkey.clone(),
                };
            }
            DvmFeedbackStatus::Job(JobStatus::PaymentRequired) => {
                if session.payment_made {
                    return;
                }
                let invoice = feedback.bolt11.clone().or_else(|| {
                    let trimmed = feedback.content.trim();
                    if trimmed.starts_with("ln") {
                        Some(trimmed.to_string())
                    } else {
                        None
                    }
                });
                if let Some(invoice) = invoice {
                    payment_request =
                        Some((invoice, feedback.amount_msats, feedback.provider_pubkey));
                } else {
                    session.lifecycle = DvmContainerLifecycle::Failed {
                        error: "payment required but invoice missing".to_string(),
                        at: Timestamp::now(),
                    };
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Error) => {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: feedback
                        .status_extra
                        .unwrap_or_else(|| "provider error".to_string()),
                    at: Timestamp::now(),
                };
            }
            _ => {}
        }
    }

    let Some((invoice, amount_msats, provider_pubkey)) = payment_request else {
        return;
    };
    let Some(wallet) = wallet.as_ref() else {
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        if let Some(session) = guard.get_mut(session_id) {
            session.lifecycle = DvmContainerLifecycle::Failed {
                error: "wallet not configured".to_string(),
                at: Timestamp::now(),
            };
        }
        return;
    };
    let amount_sats = amount_msats.map(msats_to_sats);
    let wallet = Arc::clone(wallet);
    let payment = block_on_wallet(async move { wallet.pay_invoice(&invoice, amount_sats).await });
    match payment {
        Ok(payment) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.payment_made = true;
                session.paid_amount_sats = Some(payment.amount_sats);
                session.lifecycle = DvmContainerLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: provider_pubkey,
                };
            }
        }
        Err(err) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_container_result(
    session_id: &str,
    event: &nostr::Event,
    sessions: &Arc<RwLock<HashMap<String, DvmContainerSession>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let result_event = match JobResult::from_event(event) {
        Ok(result) => result,
        Err(_) => return,
    };
    let invoice = result_event.bolt11.clone();
    let amount_sats = result_event.amount.map(msats_to_sats);

    let (response, already_paid) = {
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        let Some(session) = guard.get_mut(session_id) else {
            return;
        };
        if matches!(session.lifecycle, DvmContainerLifecycle::Failed { .. }) {
            return;
        }
        let run = match SandboxRunResult::from_job_result(&result_event) {
            Ok(run) => run,
            Err(err) => {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
                return;
            }
        };

        let cost_sats = amount_sats
            .or(session.paid_amount_sats)
            .or_else(|| {
                session
                    .accepted_quote
                    .as_ref()
                    .map(|quote| quote.price_sats)
            })
            .unwrap_or(0);
        let cost_usd = match fx.sats_to_usd(cost_sats) {
            Ok(cost_usd) => cost_usd,
            Err(err) => {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
                return;
            }
        };

        let duration_ms = Timestamp::now()
            .as_millis()
            .saturating_sub(session.submitted_at.as_millis()) as u64;
        let command_results = run
            .command_results
            .into_iter()
            .map(|cmd| CommandResult {
                command: cmd.command,
                exit_code: cmd.exit_code,
                stdout: cmd.stdout,
                stderr: cmd.stderr,
                duration_ms: cmd.duration_ms,
            })
            .collect::<Vec<_>>();
        let artifacts = run
            .artifacts
            .into_iter()
            .map(|artifact| ArtifactInfo {
                path: artifact.path,
                size_bytes: artifact.size,
                sha256: artifact.sha256,
            })
            .collect::<Vec<_>>();
        let usage = ContainerUsage {
            cpu_time_ms: run.usage.cpu_time_ms,
            peak_memory_bytes: run.usage.peak_memory_bytes,
            disk_writes_bytes: run.usage.disk_writes_bytes,
            network_bytes: run.usage.network_bytes,
        };

        let response = ContainerResponse {
            session_id: session_id.to_string(),
            exit_code: Some(run.exit_code),
            stdout: run.stdout,
            stderr: run.stderr,
            command_results,
            artifacts,
            usage,
            cost_usd,
            reserved_usd: session.request.max_cost_usd.unwrap_or(0),
            duration_ms,
            provider_id: "dvm".to_string(),
        };
        session.result = Some(response.clone());
        if invoice.is_some() {
            session.lifecycle = DvmContainerLifecycle::PendingSettlement {
                result_at: Timestamp::now(),
                invoice: invoice.clone(),
            };
        } else {
            session.lifecycle = DvmContainerLifecycle::Settled {
                settled_at: Timestamp::now(),
            };
        }
        (response, session.payment_made)
    };

    if invoice.is_none() || already_paid {
        if already_paid {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.lifecycle = DvmContainerLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        return;
    }

    let Some(wallet) = wallet.as_ref() else {
        let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
        if let Some(session) = guard.get_mut(session_id) {
            session.lifecycle = DvmContainerLifecycle::Failed {
                error: "wallet not configured".to_string(),
                at: Timestamp::now(),
            };
        }
        return;
    };
    let invoice = invoice.unwrap();
    let wallet = Arc::clone(wallet);
    let payment = block_on_wallet(async move { wallet.pay_invoice(&invoice, amount_sats).await });
    match payment {
        Ok(payment) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.payment_made = true;
                session.paid_amount_sats = Some(payment.amount_sats);
                session.result = Some(response);
                session.lifecycle = DvmContainerLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        Err(err) => {
            let mut guard = sessions.write().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.get_mut(session_id) {
                session.lifecycle = DvmContainerLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn join_workdir(repo_subdir: &Option<String>, workdir: &Option<String>) -> Option<String> {
    match (repo_subdir.as_ref(), workdir.as_ref()) {
        (Some(base), Some(extra)) => Some(format!(
            "{}/{}",
            base.trim_end_matches('/'),
            extra.trim_start_matches('/')
        )),
        (Some(base), None) => Some(base.clone()),
        (None, Some(extra)) => Some(extra.clone()),
        (None, None) => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct AsyncExecutor {
    runtime: Arc<tokio::runtime::Runtime>,
}

#[cfg(not(target_arch = "wasm32"))]
impl AsyncExecutor {
    fn new() -> Result<Self, ContainerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    fn runtime(&self) -> Arc<tokio::runtime::Runtime> {
        self.runtime.clone()
    }

    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.runtime.block_on(fut)
    }

    fn spawn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }
}

