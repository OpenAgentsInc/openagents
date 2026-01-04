//! PylonCore - Shared state and runtimes for CLI and GUI modes

use crate::bridge_manager::BridgeManager;
use crate::fm_runtime::{FmEvent, FmRuntime};
use crate::frlm_integration::FrlmIntegration;
use crate::nostr_runtime::{NostrEvent, NostrRuntime};
use crate::state::{
    ChatMessage, FmConnectionStatus, FmStreamStatus, FmVizState, Job, JobStatus,
    NostrConnectionStatus, PendingInvoice,
};
use crate::wallet_runtime::{WalletEvent, WalletRuntime, SATS_PER_JOB};
use spark::Network;
use std::time::{SystemTime, UNIX_EPOCH};

/// Core Pylon state shared between CLI and GUI modes
pub struct PylonCore {
    #[allow(dead_code)]
    pub bridge: BridgeManager,
    pub state: FmVizState,
    pub fm_runtime: FmRuntime,
    pub nostr_runtime: NostrRuntime,
    pub wallet_runtime: WalletRuntime,
    /// FRLM Conductor integration
    pub frlm: FrlmIntegration,
}

impl PylonCore {
    /// Create a new PylonCore, starting bridge and runtimes
    pub fn new() -> Self {
        // Start FM Bridge process
        let mut bridge = BridgeManager::new();
        let mut state = FmVizState::new();

        match bridge.start() {
            Ok(()) => {
                state.bridge_status_message = Some("Starting FM Bridge...".to_string());

                match bridge.wait_ready() {
                    Ok(()) => {
                        // Set the URL for FMClient
                        // SAFETY: We're in single-threaded init
                        unsafe { std::env::set_var("FM_BRIDGE_URL", bridge.url()) };
                        state.bridge_url = bridge.url().replace("http://", "");
                        state.bridge_status_message = Some("FM Bridge running".to_string());
                    }
                    Err(e) => {
                        state.connection_status = FmConnectionStatus::Error;
                        state.bridge_status_message = Some(format!("Bridge startup failed: {}", e));
                        state.error_message = Some(e.to_string());
                    }
                }
            }
            Err(e) => {
                state.connection_status = FmConnectionStatus::Error;
                state.bridge_status_message = Some(format!("Bridge not found: {}", e));
                state.error_message = Some(e.to_string());
            }
        }

        // Create runtimes
        let nostr_runtime = NostrRuntime::new();
        state.pubkey = Some(nostr_runtime.pubkey().to_string());

        let fm_runtime = FmRuntime::new();

        // Create wallet runtime (testnet by default)
        let wallet_runtime = WalletRuntime::new(Network::Testnet);

        // Create FRLM integration
        let mut frlm = FrlmIntegration::new();
        frlm.init(&nostr_runtime, Some(&state.bridge_url));

        Self {
            bridge,
            state,
            fm_runtime,
            nostr_runtime,
            wallet_runtime,
            frlm,
        }
    }

    /// Poll all events (non-blocking). Returns true if any events were processed.
    pub fn poll(&mut self) -> bool {
        let mut processed = false;

        // Poll FM events
        while let Ok(event) = self.fm_runtime.event_rx.try_recv() {
            processed = true;
            match event {
                FmEvent::Connected { model_available, latency_ms } => {
                    self.state.on_connected(model_available, latency_ms);
                }
                FmEvent::ConnectionFailed(error) => {
                    self.state.on_connection_failed(error);
                }
                FmEvent::FirstToken { text, ttft_ms } => {
                    self.state.on_first_token(&text, ttft_ms);
                }
                FmEvent::Token { text } => {
                    self.state.on_token(&text);
                }
                FmEvent::StreamComplete => {
                    self.state.on_stream_complete();

                    // Publish result if serving a job
                    if let Some(job_id) = self.state.current_job_id.take() {
                        let result = self.state.token_stream.clone();
                        if let Some(job) = self.state.jobs.iter().find(|j| j.id == job_id) {
                            self.nostr_runtime.publish_job_result(
                                &job_id,
                                &job.from_pubkey,
                                &result,
                            );

                            // Create invoice for payment
                            if self.state.wallet_connected {
                                let description = format!("NIP-90 job {}", &job_id[..8.min(job_id.len())]);
                                self.wallet_runtime.create_invoice(&job_id, &description);
                            }
                        }
                        self.state.update_job_status(&job_id, JobStatus::Complete);
                        self.state.jobs_served += 1;
                        // Pending earnings until invoice is paid
                        self.state.pending_earnings += SATS_PER_JOB;
                    }
                }
                FmEvent::StreamError(error) => {
                    self.state.on_stream_error(error.clone());

                    if let Some(job_id) = self.state.current_job_id.take() {
                        self.state.update_job_status(&job_id, JobStatus::Failed);
                    }
                }
            }
        }

        // Poll Nostr events
        while let Ok(event) = self.nostr_runtime.event_rx.try_recv() {
            processed = true;
            match event {
                NostrEvent::Connected => {
                    self.state.nostr_status = NostrConnectionStatus::Connected;
                    // Don't subscribe yet - wait for auth (relay requires it)
                }
                NostrEvent::Authenticated => {
                    self.state.nostr_status = NostrConnectionStatus::Authenticated;
                    // Now we can subscribe (after auth)
                    self.nostr_runtime.subscribe_jobs();
                    self.nostr_runtime.subscribe_chat("openagents-providers");
                    self.nostr_runtime.create_or_find_channel("openagents-providers");
                }
                NostrEvent::ConnectionFailed(error) => {
                    self.state.nostr_status = NostrConnectionStatus::Error;
                    self.state.error_message = Some(error);
                }
                NostrEvent::AuthChallenge(challenge) => {
                    // Respond to NIP-42 auth challenge
                    self.nostr_runtime.authenticate(&challenge);
                }
                NostrEvent::JobRequest { id, pubkey, prompt, created_at } => {
                    let job = Job {
                        id: id.clone(),
                        _prompt: prompt.clone(),
                        from_pubkey: pubkey,
                        status: JobStatus::Pending,
                        result: None,
                        _created_at: created_at,
                        is_outgoing: false,
                    };
                    self.state.add_job(job);

                    // Auto-serve if not busy
                    if self.state.current_job_id.is_none() && !self.state.is_streaming() {
                        self.state.current_job_id = Some(id.clone());
                        self.state.update_job_status(&id, JobStatus::Serving);
                        self.state.on_stream_start(&prompt);
                        self.fm_runtime.stream(prompt);
                    }
                }
                NostrEvent::JobResult { _id: _, request_id, _pubkey: _, content, amount_msats, bolt11 } => {
                    if self.state.pending_requests.remove(&request_id).is_some() {
                        self.state.token_stream = content.clone();
                        self.state.stream_status = FmStreamStatus::Complete;

                        if let Some(job) = self.state.jobs.iter_mut().find(|j| j.id == request_id) {
                            job.status = JobStatus::Complete;
                            job.result = Some(content);
                        }

                        // Pay the invoice if one was included
                        if let Some(invoice) = bolt11 {
                            if self.state.wallet_connected {
                                let amount_sats = amount_msats.unwrap_or(0) / 1000;
                                if self.state.balance_sats >= amount_sats {
                                    self.wallet_runtime.pay_invoice(&invoice);
                                } else {
                                    self.state.error_message = Some(format!(
                                        "Insufficient balance: {} sats needed, {} available",
                                        amount_sats, self.state.balance_sats
                                    ));
                                }
                            }
                        }
                    }
                }
                NostrEvent::ChatMessage { id, pubkey, content, created_at } => {
                    let is_self = self.state.pubkey.as_deref() == Some(&pubkey);
                    let msg = ChatMessage {
                        _id: id,
                        author: FmVizState::short_pubkey(&pubkey),
                        content,
                        _timestamp: created_at,
                        is_self,
                    };
                    self.state.add_chat_message(msg);
                }
                NostrEvent::Published { _event_id: _ } => {}
                NostrEvent::PublishFailed { error } => {
                    self.state.error_message = Some(error);
                }
                NostrEvent::ChannelFound { channel_id, _name: _ } => {
                    self.state.channel_id = Some(channel_id.clone());
                    self.nostr_runtime.subscribe_chat(&channel_id);
                }
                NostrEvent::JobBatchPublished { job_mappings } => {
                    // FRLM: Batch of jobs published to swarm
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    for (_local_id, job_id) in &job_mappings {
                        self.state.pending_requests.insert(job_id.clone(), crate::state::PendingRequest {
                            _prompt: String::from("[FRLM batch query]"),
                            _requested_at: now,
                        });
                    }
                }
                NostrEvent::JobBatchFailed { local_id, error } => {
                    // FRLM: Batch job failed to publish
                    self.state.error_message = Some(format!(
                        "Job batch failed for {}: {}", local_id, error
                    ));
                }
            }
        }

        // Poll wallet events
        while let Ok(event) = self.wallet_runtime.event_rx.try_recv() {
            processed = true;
            match event {
                WalletEvent::Initialized { balance_sats, _spark_address: _ } => {
                    self.state.wallet_connected = true;
                    self.state.balance_sats = balance_sats;
                }
                WalletEvent::InitFailed(error) => {
                    self.state.wallet_connected = false;
                    eprintln!("Wallet init failed: {}", error);
                }
                WalletEvent::BalanceUpdated { balance_sats } => {
                    self.state.balance_sats = balance_sats;
                }
                WalletEvent::InvoiceCreated { job_id, bolt11, amount_sats } => {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    self.state.pending_invoices.insert(
                        job_id,
                        PendingInvoice {
                            bolt11,
                            amount_sats,
                            created_at: now,
                        },
                    );
                }
                WalletEvent::InvoiceCreationFailed { job_id, error } => {
                    eprintln!("Invoice creation failed for job {}: {}", job_id, error);
                }
                WalletEvent::PaymentReceived { _payment_id: _, amount_sats } => {
                    // Move from pending to confirmed
                    if self.state.pending_earnings >= amount_sats {
                        self.state.pending_earnings -= amount_sats;
                    }
                    self.state.balance_sats += amount_sats;
                }
                WalletEvent::PaymentSent { _payment_id: _, amount_sats } => {
                    // Balance already updated in wallet, just log
                    eprintln!("Payment sent: {} sats", amount_sats);
                }
                WalletEvent::PaymentFailed { error } => {
                    self.state.error_message = Some(format!("Payment failed: {}", error));
                }
            }
        }

        // Periodically poll wallet for payments (every ~5 seconds based on poll rate)
        // This is a simple approach - could be improved with a timer
        static mut POLL_COUNTER: u32 = 0;
        unsafe {
            POLL_COUNTER += 1;
            if POLL_COUNTER % 50 == 0 {
                // ~5 seconds at 100ms poll rate
                self.wallet_runtime.poll_payments();
            }
        }

        // Poll FRLM trace events and update UI state
        if self.frlm.poll(&mut self.state) {
            processed = true;
        }

        processed
    }

    /// Connect to the Nostr relay
    pub fn connect_nostr(&self) {
        self.nostr_runtime.connect(&self.state.relay_url);
    }

    /// Connect to the FM Bridge
    pub fn connect_bridge(&self) {
        self.fm_runtime.connect();
    }
}

impl Default for PylonCore {
    fn default() -> Self {
        Self::new()
    }
}
