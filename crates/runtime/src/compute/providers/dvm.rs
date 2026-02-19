/// NIP-90 DVM provider for decentralized compute.
#[cfg(not(target_arch = "wasm32"))]
pub struct DvmProvider {
    agent_id: AgentId,
    transport: Arc<dyn DvmTransport>,
    signer: Arc<dyn SigningService>,
    wallet: Option<Arc<dyn WalletService>>,
    fx: Arc<FxRateCache>,
    executor: Executor,
    jobs: Arc<RwLock<HashMap<String, DvmJobState>>>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
#[expect(dead_code)]
struct DvmJobState {
    job_id: String,
    request_event_id: String,
    request_kind: u16,
    request: ComputeRequest,
    submitted_at: Timestamp,
    lifecycle: DvmLifecycle,
    quotes: Vec<DvmQuote>,
    accepted_quote: Option<DvmQuote>,
    result: Option<ComputeResponse>,
    partials: VecDeque<ComputeChunk>,
    payment_made: bool,
    paid_amount_sats: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DvmProvider {
    /// Create a new DVM provider using Nostr relays.
    pub fn new(
        agent_id: AgentId,
        relays: Vec<String>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ComputeError> {
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

    /// Create a DVM provider with a custom transport (tests).
    pub(crate) fn with_transport(
        agent_id: AgentId,
        transport: Arc<dyn DvmTransport>,
        signer: Arc<dyn SigningService>,
        wallet: Option<Arc<dyn WalletService>>,
        fx_source: FxSource,
        fx_cache_secs: u64,
    ) -> Result<Self, ComputeError> {
        let executor = Executor::new()?;
        let runtime = executor.runtime();
        executor
            .block_on(transport.connect())
            .map_err(ComputeError::ProviderError)?;
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
            jobs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn map_kind(kind: &ComputeKind) -> Result<u16, ComputeError> {
        match kind {
            ComputeKind::Chat | ComputeKind::Complete => Ok(KIND_JOB_TEXT_GENERATION),
            ComputeKind::ImageGenerate => Ok(KIND_JOB_IMAGE_GENERATION),
            ComputeKind::Transcribe => Ok(KIND_JOB_SPEECH_TO_TEXT),
            other => Err(ComputeError::UnsupportedKind(format!("{:?}", other))),
        }
    }

    fn build_job_request(
        &self,
        request: &ComputeRequest,
        kind: u16,
    ) -> Result<JobRequest, ComputeError> {
        let prompt = match request.kind {
            ComputeKind::Chat => parse_messages(&request.input),
            ComputeKind::Complete | ComputeKind::ImageGenerate | ComputeKind::Transcribe => {
                parse_prompt(&request.input)
            }
            _ => None,
        }
        .ok_or_else(|| ComputeError::InvalidRequest("missing prompt".to_string()))?;

        let mut job = JobRequest::new(kind)
            .map_err(|err| ComputeError::InvalidRequest(err.to_string()))?
            .add_input(JobInput::text(prompt))
            .add_param("model", request.model.clone());

        if let Some(obj) = request.input.as_object() {
            if let Some(max_tokens) = obj.get("max_tokens").and_then(|v| v.as_u64()) {
                job = job.add_param("max_tokens", max_tokens.to_string());
            }
            if let Some(temp) = obj.get("temperature").and_then(|v| v.as_f64()) {
                job = job.add_param("temperature", temp.to_string());
            }
            if let Some(top_p) = obj.get("top_p").and_then(|v| v.as_f64()) {
                job = job.add_param("top_p", top_p.to_string());
            }
            if let Some(stop) = obj.get("stop").and_then(|v| v.as_array()) {
                for (idx, item) in stop.iter().enumerate() {
                    if let Some(value) = item.as_str() {
                        job = job.add_param(format!("stop_{}", idx), value.to_string());
                    }
                }
            }
        }

        for relay in self.transport.relays() {
            job = job.add_relay(relay);
        }

        let max_cost_usd = request.max_cost_usd.unwrap_or(100_000);
        let bid_msats = bid_msats_for_max_cost(&self.fx, max_cost_usd)
            .map_err(ComputeError::ProviderError)?;
        job = job.with_bid(bid_msats);

        Ok(job)
    }

    fn sign_event(
        &self,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> Result<nostr::Event, ComputeError> {
        sign_dvm_event(&*self.signer, &self.agent_id, kind, tags, content)
            .map_err(ComputeError::ProviderError)
    }

    fn spawn_quote_manager(&self, job_id: String) {
        let jobs = self.jobs.clone();
        let transport = self.transport.clone();
        let signer = self.signer.clone();
        let agent_id = self.agent_id.clone();
        let executor = self.executor.clone();

        executor.spawn(async move {
            tokio::time::sleep(DVM_QUOTE_WINDOW).await;

            let (request_event_id, quote) = {
                let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
                let job = match guard.get_mut(&job_id) {
                    Some(job) => job,
                    None => return,
                };
                if !matches!(job.lifecycle, DvmLifecycle::AwaitingQuotes { .. }) {
                    return;
                }
                let best = match job
                    .quotes
                    .iter()
                    .min_by_key(|quote| quote.price_usd)
                    .cloned()
                {
                    Some(best) => best,
                    None => {
                        job.lifecycle = DvmLifecycle::Failed {
                            error: "no quotes received".to_string(),
                            at: Timestamp::now(),
                        };
                        return;
                    }
                };
                job.accepted_quote = Some(best.clone());
                job.lifecycle = DvmLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: best.provider_pubkey.clone(),
                };
                (job.request_event_id.clone(), best)
            };

            let tags = vec![
                vec!["e".to_string(), request_event_id],
                vec!["e".to_string(), quote.event_id.clone()],
                vec!["p".to_string(), quote.provider_pubkey.clone()],
                vec!["status".to_string(), "processing".to_string()],
            ];

            let event =
                match sign_dvm_event(&*signer, &agent_id, KIND_JOB_FEEDBACK, tags, String::new())
                {
                    Ok(event) => event,
                    Err(_) => return,
                };
            let _ = transport.publish(event).await;
        });
    }

    fn subscribe_job_events(
        &self,
        job_id: String,
        request_event_id: String,
        request_kind: u16,
    ) -> Result<(), ComputeError> {
        let result_kind = get_result_kind(request_kind)
            .ok_or_else(|| ComputeError::ProviderError("invalid job kind".to_string()))?;
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
        let subscription_id = format!("dvm-job-{}", request_event_id);
        let mut rx = self
            .executor
            .block_on(self.transport.subscribe(&subscription_id, &filters))
            .map_err(ComputeError::ProviderError)?;

        let jobs = self.jobs.clone();
        let fx = self.fx.clone();
        let wallet = self.wallet.clone();
        let request_model = {
            let guard = jobs.read().unwrap_or_else(|e| e.into_inner());
            guard
                .get(&job_id)
                .map(|job| job.request.model.clone())
                .unwrap_or_default()
        };

        self.executor.spawn(async move {
            while let Some(event) = rx.recv().await {
                if event.kind == result_kind {
                    handle_dvm_result(&job_id, &request_model, &event, &jobs, &fx, &wallet);
                } else if event.kind == KIND_JOB_FEEDBACK {
                    if let Some(feedback) = parse_feedback_event(&event) {
                        handle_dvm_feedback(&job_id, feedback, &jobs, &fx, &wallet);
                    }
                }
            }
        });

        Ok(())
    }

    fn query_handlers(&self, timeout: Duration) -> Result<Vec<HandlerInfo>, ComputeError> {
        let filters = vec![serde_json::json!({
            "kinds": [KIND_HANDLER_INFO],
            "limit": 100
        })];
        let events = self
            .executor
            .block_on(self.transport.query(&filters, timeout))
            .map_err(ComputeError::ProviderError)?;
        let mut handlers = Vec::new();
        for event in events {
            if let Ok(handler) = HandlerInfo::from_event(&event) {
                if handler.handler_type == HandlerType::ComputeProvider {
                    handlers.push(handler);
                }
            }
        }
        Ok(handlers)
    }

    /// List available handler info events for compute providers.
    pub fn list_handlers(&self, timeout: Duration) -> Result<Vec<HandlerInfo>, ComputeError> {
        self.query_handlers(timeout)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ComputeProvider for DvmProvider {
    fn id(&self) -> &str {
        "dvm"
    }

    fn info(&self) -> ProviderInfo {
        let handlers = self.query_handlers(Duration::from_secs(2)).unwrap_or_default();
        let mut models = Vec::new();
        for handler in handlers {
            for (key, value) in handler.custom_tags {
                if key == "model" {
                    models.push(ModelInfo {
                        id: value.clone(),
                        name: value,
                        context_length: None,
                        capabilities: vec![ComputeKind::Chat, ComputeKind::Complete],
                        pricing: None,
                    });
                }
            }
        }
        ProviderInfo {
            id: "dvm".to_string(),
            name: "NIP-90 DVM Network".to_string(),
            models,
            capabilities: vec![
                ComputeKind::Chat,
                ComputeKind::Complete,
                ComputeKind::ImageGenerate,
                ComputeKind::Transcribe,
            ],
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 2000,
                tokens_per_sec: None,
                measured: false,
            },
            region: None,
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

    fn supports_model(&self, model: &str) -> bool {
        self.query_handlers(Duration::from_secs(2))
            .map(|handlers| {
                handlers.iter().any(|handler| {
                    handler
                        .custom_tags
                        .iter()
                        .any(|(key, value)| key == "model" && value == model)
                })
            })
            .unwrap_or(false)
    }

    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        if self.wallet.is_none() {
            return Err(ComputeError::ProviderError(
                "wallet not configured".to_string(),
            ));
        }

        let kind = Self::map_kind(&request.kind)?;
        let job_request = self.build_job_request(&request, kind)?;
        let event = self.sign_event(
            job_request.kind,
            job_request.to_tags(),
            job_request.content.clone(),
        )?;
        let event_id = event.id.clone();

        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ComputeError::ProviderError)?;

        let job_id = uuid::Uuid::new_v4().to_string();
        let now = Timestamp::now();
        self.jobs.write().unwrap_or_else(|e| e.into_inner()).insert(
            job_id.clone(),
            DvmJobState {
                job_id: job_id.clone(),
                request_event_id: event_id.clone(),
                request_kind: kind,
                request: request.clone(),
                submitted_at: now,
                lifecycle: DvmLifecycle::AwaitingQuotes {
                    since: now,
                    timeout_at: Timestamp::from_millis(
                        now.as_millis() + DVM_QUOTE_WINDOW.as_millis() as u64,
                    ),
                },
                quotes: Vec::new(),
                accepted_quote: None,
                result: None,
                partials: VecDeque::new(),
                payment_made: false,
                paid_amount_sats: None,
            },
        );

        self.subscribe_job_events(job_id.clone(), event_id, kind)?;
        self.spawn_quote_manager(job_id.clone());
        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        let guard = self.jobs.read().unwrap_or_else(|e| e.into_inner());
        let job = guard.get(job_id)?;
        Some(match &job.lifecycle {
            DvmLifecycle::AwaitingQuotes { .. } => JobState::Pending {
                submitted_at: job.submitted_at,
            },
            DvmLifecycle::Processing { accepted_at, .. } => JobState::Running {
                started_at: *accepted_at,
            },
            DvmLifecycle::PendingSettlement { .. } => JobState::Running {
                started_at: job.submitted_at,
            },
            DvmLifecycle::Settled { .. } => {
                job.result
                    .clone()
                    .map(JobState::Complete)
                    .unwrap_or(JobState::Running {
                        started_at: job.submitted_at,
                    })
            }
            DvmLifecycle::Failed { error, at } => JobState::Failed {
                error: error.clone(),
                at: *at,
            },
        })
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut guard = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = guard.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        Ok(job.partials.pop_front())
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let (request_event_id, request_kind) = {
            let mut guard = self.jobs.write().unwrap_or_else(|e| e.into_inner());
            let job = guard.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
            job.lifecycle = DvmLifecycle::Failed {
                error: "cancelled".to_string(),
                at: Timestamp::now(),
            };
            (job.request_event_id.clone(), job.request_kind)
        };

        let tags = create_deletion_tags(&[request_event_id.as_str()], Some(request_kind));
        let event = self.sign_event(DELETION_REQUEST_KIND, tags, String::new())?;
        self.executor
            .block_on(self.transport.publish(event))
            .map_err(ComputeError::ProviderError)?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_feedback(
    job_id: &str,
    feedback: crate::dvm::DvmFeedback,
    jobs: &Arc<RwLock<HashMap<String, DvmJobState>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let mut payment_request = None;

    {
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        let Some(job) = guard.get_mut(job_id) else {
            return;
        };
        if matches!(
            job.lifecycle,
            DvmLifecycle::Failed { .. } | DvmLifecycle::Settled { .. }
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
                            job.lifecycle = DvmLifecycle::Failed {
                                error: err.to_string(),
                                at: Timestamp::now(),
                            };
                            return;
                        }
                    };
                    let quote = DvmQuote {
                        provider_pubkey: feedback.provider_pubkey.clone(),
                        price_sats,
                        price_usd,
                        event_id: feedback.event_id.clone(),
                    };
                    if let Some(existing) = job
                        .quotes
                        .iter_mut()
                        .find(|q| q.provider_pubkey == quote.provider_pubkey)
                    {
                        if quote.price_usd < existing.price_usd {
                            *existing = quote;
                        }
                    } else {
                        job.quotes.push(quote);
                    }
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Partial) => {
                let chunk = ComputeChunk {
                    job_id: job_id.to_string(),
                    delta: serde_json::json!({ "text": feedback.content }),
                    finish_reason: None,
                    usage: None,
                };
                job.partials.push_back(chunk);
            }
            DvmFeedbackStatus::Job(JobStatus::Processing) => {
                job.lifecycle = DvmLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: feedback.provider_pubkey.clone(),
                };
            }
            DvmFeedbackStatus::Job(JobStatus::PaymentRequired) => {
                if job.payment_made {
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
                    job.lifecycle = DvmLifecycle::Failed {
                        error: "payment required but invoice missing".to_string(),
                        at: Timestamp::now(),
                    };
                }
            }
            DvmFeedbackStatus::Job(JobStatus::Error) => {
                job.lifecycle = DvmLifecycle::Failed {
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
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = guard.get_mut(job_id) {
            job.lifecycle = DvmLifecycle::Failed {
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
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.payment_made = true;
                job.paid_amount_sats = Some(payment.amount_sats);
                job.lifecycle = DvmLifecycle::Processing {
                    accepted_at: Timestamp::now(),
                    provider: provider_pubkey,
                };
            }
        }
        Err(err) => {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.lifecycle = DvmLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_dvm_result(
    job_id: &str,
    model: &str,
    event: &nostr::Event,
    jobs: &Arc<RwLock<HashMap<String, DvmJobState>>>,
    fx: &Arc<FxRateCache>,
    wallet: &Option<Arc<dyn WalletService>>,
) {
    let result = match JobResult::from_event(event) {
        Ok(result) => result,
        Err(_) => return,
    };
    let invoice = result.bolt11.clone();
    let amount_sats = result.amount.map(msats_to_sats);

    let (response, already_paid) = {
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        let Some(job) = guard.get_mut(job_id) else {
            return;
        };
        if matches!(job.lifecycle, DvmLifecycle::Failed { .. }) {
            return;
        }
        let cost_sats = amount_sats
            .or(job.paid_amount_sats)
            .or_else(|| job.accepted_quote.as_ref().map(|quote| quote.price_sats))
            .unwrap_or(0);
        let cost_usd = match fx.sats_to_usd(cost_sats) {
            Ok(cost_usd) => cost_usd,
            Err(err) => {
                job.lifecycle = DvmLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
                return;
            }
        };
        let output = serde_json::from_str(&result.content)
            .unwrap_or_else(|_| serde_json::json!({ "text": result.content }));
        let latency_ms = Timestamp::now()
            .as_millis()
            .saturating_sub(job.submitted_at.as_millis()) as u64;
        let response = ComputeResponse {
            job_id: job_id.to_string(),
            output,
            usage: None,
            cost_usd,
            latency_ms,
            provider_id: "dvm".to_string(),
            model: model.to_string(),
        };
        job.result = Some(response.clone());
        if invoice.is_some() {
            job.lifecycle = DvmLifecycle::PendingSettlement {
                result_at: Timestamp::now(),
                invoice: invoice.clone(),
            };
        } else {
            job.lifecycle = DvmLifecycle::Settled {
                settled_at: Timestamp::now(),
            };
        }
        (response, job.payment_made)
    };

    if invoice.is_none() || already_paid {
        if already_paid {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.lifecycle = DvmLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        return;
    }

    let Some(wallet) = wallet.as_ref() else {
        let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = guard.get_mut(job_id) {
            job.lifecycle = DvmLifecycle::Failed {
                error: "wallet not configured".to_string(),
                at: Timestamp::now(),
            };
        }
        return;
    };
    let Some(invoice) = invoice else {
        return;
    };
    let wallet = Arc::clone(wallet);
    let payment = block_on_wallet(async move { wallet.pay_invoice(&invoice, amount_sats).await });
    match payment {
        Ok(payment) => {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.payment_made = true;
                job.paid_amount_sats = Some(payment.amount_sats);
                job.result = Some(response);
                job.lifecycle = DvmLifecycle::Settled {
                    settled_at: Timestamp::now(),
                };
            }
        }
        Err(err) => {
            let mut guard = jobs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = guard.get_mut(job_id) {
                job.lifecycle = DvmLifecycle::Failed {
                    error: err.to_string(),
                    at: Timestamp::now(),
                };
            }
        }
    }
}
