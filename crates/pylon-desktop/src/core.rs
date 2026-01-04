//! PylonCore - Shared state and runtimes for CLI and GUI modes

use crate::bridge_manager::BridgeManager;
use crate::fm_runtime::{FmEvent, FmRuntime};
use crate::nostr_runtime::{NostrEvent, NostrRuntime};
use crate::state::{
    ChatMessage, FmConnectionStatus, FmStreamStatus, FmVizState, Job, JobStatus,
    NostrConnectionStatus,
};

/// Core Pylon state shared between CLI and GUI modes
pub struct PylonCore {
    pub bridge: BridgeManager,
    pub state: FmVizState,
    pub fm_runtime: FmRuntime,
    pub nostr_runtime: NostrRuntime,
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

        Self {
            bridge,
            state,
            fm_runtime,
            nostr_runtime,
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
                        }
                        self.state.update_job_status(&job_id, JobStatus::Complete);
                        self.state.jobs_served += 1;
                        self.state.credits += 1;
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
                    self.nostr_runtime.subscribe_jobs();
                    self.nostr_runtime.subscribe_chat("openagents-providers");
                }
                NostrEvent::Authenticated => {
                    self.state.nostr_status = NostrConnectionStatus::Authenticated;
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
                        prompt: prompt.clone(),
                        from_pubkey: pubkey,
                        status: JobStatus::Pending,
                        result: None,
                        created_at,
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
                NostrEvent::JobResult { id: _, request_id, pubkey: _, content } => {
                    if self.state.pending_requests.remove(&request_id).is_some() {
                        self.state.token_stream = content.clone();
                        self.state.stream_status = FmStreamStatus::Complete;

                        if let Some(job) = self.state.jobs.iter_mut().find(|j| j.id == request_id) {
                            job.status = JobStatus::Complete;
                            job.result = Some(content);
                        }
                    }
                }
                NostrEvent::ChatMessage { id, pubkey, content, created_at } => {
                    let is_self = self.state.pubkey.as_deref() == Some(&pubkey);
                    let msg = ChatMessage {
                        id,
                        author: FmVizState::short_pubkey(&pubkey),
                        content,
                        timestamp: created_at,
                        is_self,
                    };
                    self.state.add_chat_message(msg);
                }
                NostrEvent::Published { event_id: _ } => {}
                NostrEvent::PublishFailed { error } => {
                    self.state.error_message = Some(error);
                }
                NostrEvent::ChannelFound { channel_id, name: _ } => {
                    self.state.channel_id = Some(channel_id.clone());
                    self.nostr_runtime.subscribe_chat(&channel_id);
                }
            }
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
