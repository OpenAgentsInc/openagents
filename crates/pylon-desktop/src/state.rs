//! FM Bridge and Nostr state for visualization

use std::collections::HashMap;
use web_time::Instant;

#[derive(Clone, Copy, PartialEq)]
pub enum FmConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FmStreamStatus {
    Idle,
    Streaming,
    Complete,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NostrConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Authenticated,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
pub enum JobStatus {
    Pending,
    Serving,
    Complete,
    Failed,
}

#[derive(Clone, Copy, PartialEq)]
pub enum InputFocus {
    Jobs,
    Chat,
    Prompt,
}

pub struct TranscriptMessage {
    pub _role: &'static str,
    pub _content: String,
}

/// A NIP-90 job (request or our result)
#[derive(Clone)]
pub struct Job {
    pub id: String,
    pub _prompt: String,
    pub from_pubkey: String,
    pub status: JobStatus,
    pub result: Option<String>,
    pub _created_at: u64,
    pub is_outgoing: bool,  // true = we requested, false = we serve
}

/// A pending request we made (waiting for result)
#[derive(Clone)]
pub struct PendingRequest {
    pub _prompt: String,
    pub _requested_at: u64,
}

/// A pending invoice we created for a job we served
#[derive(Clone)]
#[allow(dead_code)]
pub struct PendingInvoice {
    pub bolt11: String,      // The Lightning invoice
    pub amount_sats: u64,    // Amount in satoshis
    pub created_at: u64,     // Unix timestamp
}

/// A NIP-28 chat message
#[derive(Clone)]
pub struct ChatMessage {
    pub _id: String,
    pub author: String,  // npub (shortened for display)
    pub content: String,
    pub _timestamp: u64,
    pub is_self: bool,
}

// ============ FRLM (Federated RLM) State ============

/// Status of a sub-query in an FRLM run
#[derive(Clone, PartialEq)]
pub enum SubQueryDisplayStatus {
    Pending,
    Submitted { job_id: String },
    Executing { provider_id: String },
    Complete { duration_ms: u64 },
    Failed { error: String },
    Timeout,
}

/// State for an active FRLM run
#[derive(Clone)]
pub struct FrlmRunState {
    pub run_id: String,
    pub program: String,
    pub fragment_count: usize,
    pub pending_queries: usize,
    pub completed_queries: usize,
    pub budget_used_sats: u64,
    pub budget_remaining_sats: u64,
    pub started_at: u64,
}

impl FrlmRunState {
    pub fn new(run_id: String, program: String, fragment_count: usize, budget_limit_sats: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            run_id,
            program,
            fragment_count,
            pending_queries: 0,
            completed_queries: 0,
            budget_used_sats: 0,
            budget_remaining_sats: budget_limit_sats,
            started_at: now,
        }
    }
}

pub struct FmVizState {
    // Connection
    pub connection_status: FmConnectionStatus,
    pub bridge_url: String,
    pub model_available: bool,
    pub ping_latency_ms: Option<u32>,
    pub error_message: Option<String>,
    pub bridge_status_message: Option<String>,

    // Streaming
    pub stream_status: FmStreamStatus,
    pub token_stream: String,
    pub tokens_per_sec: f32,
    pub token_count: u32,
    pub ttft_ms: Option<u64>,

    // Timing
    stream_start: Option<Instant>,
    last_token_time: Option<Instant>,

    // Session
    pub _session_id: Option<String>,
    pub turn_count: u32,
    pub transcript: Vec<TranscriptMessage>,

    // Input
    pub prompt_input: String,
    pub cursor_pos: usize,
    pub selection: Option<(usize, usize)>, // (start, end)
    pub input_focus: InputFocus,

    // Viz history
    pub token_history: Vec<f32>,

    // Nostr connection
    pub nostr_status: NostrConnectionStatus,
    pub relay_url: String,
    pub pubkey: Option<String>,  // Our npub

    // NIP-90 Jobs
    pub jobs: Vec<Job>,
    pub current_job_id: Option<String>,  // Job we're currently serving
    pub jobs_served: u32,
    pub jobs_requested: u32,
    pub pending_requests: HashMap<String, PendingRequest>,  // event_id -> our request

    // Wallet (real Bitcoin sats via Spark)
    pub balance_sats: u64,           // Current wallet balance
    pub pending_earnings: u64,       // Sats earned but not yet confirmed
    pub wallet_connected: bool,      // Whether wallet initialized successfully
    pub pending_invoices: HashMap<String, PendingInvoice>,  // job_id -> invoice we created

    // NIP-28 Chat
    pub chat_messages: Vec<ChatMessage>,
    pub chat_input: String,
    pub chat_cursor: usize,
    pub channel_id: Option<String>,

    // FRLM (Federated RLM) state
    pub frlm_active_run: Option<FrlmRunState>,
    pub frlm_subquery_status: HashMap<String, SubQueryDisplayStatus>,
    pub frlm_runs_completed: u32,
    pub frlm_total_cost_sats: u64,
}

impl FmVizState {
    pub fn new() -> Self {
        Self {
            connection_status: FmConnectionStatus::Disconnected,
            bridge_url: "localhost:11435".to_string(),
            model_available: false,
            ping_latency_ms: None,
            error_message: None,
            bridge_status_message: None,

            stream_status: FmStreamStatus::Idle,
            token_stream: String::new(),
            tokens_per_sec: 0.0,
            token_count: 0,
            ttft_ms: None,

            stream_start: None,
            last_token_time: None,

            _session_id: None,
            turn_count: 0,
            transcript: Vec::new(),

            prompt_input: String::new(),
            cursor_pos: 0,
            selection: None,
            input_focus: InputFocus::Prompt,

            token_history: vec![0.0; 50],

            nostr_status: NostrConnectionStatus::Disconnected,
            relay_url: "wss://relay.openagents.com/".to_string(),
            pubkey: None,

            jobs: Vec::new(),
            current_job_id: None,
            jobs_served: 0,
            jobs_requested: 0,
            pending_requests: HashMap::new(),

            balance_sats: 0,
            pending_earnings: 0,
            wallet_connected: false,
            pending_invoices: HashMap::new(),

            chat_messages: Vec::new(),
            chat_input: String::new(),
            chat_cursor: 0,
            channel_id: None,

            frlm_active_run: None,
            frlm_subquery_status: HashMap::new(),
            frlm_runs_completed: 0,
            frlm_total_cost_sats: 0,
        }
    }

    /// Add a job to the list
    pub fn add_job(&mut self, job: Job) {
        self.jobs.insert(0, job);  // Add at front (newest first)
        if self.jobs.len() > 50 {
            self.jobs.pop();  // Keep max 50 jobs
        }
    }

    /// Update job status
    pub fn update_job_status(&mut self, job_id: &str, status: JobStatus) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            job.status = status;
        }
    }

    /// Add a chat message
    pub fn add_chat_message(&mut self, msg: ChatMessage) {
        self.chat_messages.push(msg);
        if self.chat_messages.len() > 100 {
            self.chat_messages.remove(0);  // Keep max 100 messages
        }
    }

    /// Shorten pubkey for display (first 8 chars)
    pub fn short_pubkey(pubkey: &str) -> String {
        if pubkey.len() > 8 {
            format!("{}...", &pubkey[..8])
        } else {
            pubkey.to_string()
        }
    }

    /// Called when connection succeeds
    pub fn on_connected(&mut self, model_available: bool, latency_ms: u32) {
        self.connection_status = FmConnectionStatus::Connected;
        self.model_available = model_available;
        self.ping_latency_ms = Some(latency_ms);
        self.error_message = None;
    }

    /// Called when connection fails
    pub fn on_connection_failed(&mut self, error: String) {
        self.connection_status = FmConnectionStatus::Error;
        self.error_message = Some(error);
    }

    /// Called when starting a new stream
    pub fn on_stream_start(&mut self, prompt: &str) {
        self.stream_status = FmStreamStatus::Streaming;
        self.token_stream.clear();
        self.token_count = 0;
        self.tokens_per_sec = 0.0;
        self.ttft_ms = None;
        self.stream_start = Some(Instant::now());
        self.last_token_time = None;

        // Add user message to transcript
        self.transcript.push(TranscriptMessage {
            _role: "USER",
            _content: prompt.to_string(),
        });
        self.turn_count += 1;
    }

    /// Called when first token arrives
    pub fn on_first_token(&mut self, text: &str, ttft_ms: u64) {
        self.ttft_ms = Some(ttft_ms);
        self.token_stream.push_str(text);
        self.token_count = 1;
        self.last_token_time = Some(Instant::now());

        // Update history
        self.update_history();
    }

    /// Called when subsequent token arrives
    pub fn on_token(&mut self, text: &str) {
        self.token_stream.push_str(text);
        self.token_count += 1;

        // Calculate tokens/sec
        if let Some(start) = self.stream_start {
            let elapsed = start.elapsed().as_secs_f32();
            if elapsed > 0.0 {
                // Smooth the rate
                let instant_rate = self.token_count as f32 / elapsed;
                self.tokens_per_sec = self.tokens_per_sec * 0.7 + instant_rate * 0.3;
            }
        }

        self.last_token_time = Some(Instant::now());

        // Update history
        self.update_history();
    }

    /// Called when stream completes
    pub fn on_stream_complete(&mut self) {
        self.stream_status = FmStreamStatus::Complete;

        // Add assistant message to transcript
        if !self.token_stream.is_empty() {
            self.transcript.push(TranscriptMessage {
                _role: "ASST",
                _content: self.token_stream.clone(),
            });
        }

        // Clear prompt input
        self.prompt_input.clear();
        self.cursor_pos = 0;
        self.selection = None;
    }

    /// Called when stream errors
    pub fn on_stream_error(&mut self, error: String) {
        self.stream_status = FmStreamStatus::Error;
        self.error_message = Some(error);
    }

    /// Update token history for visualization
    fn update_history(&mut self) {
        self.token_history.remove(0);
        self.token_history.push((self.tokens_per_sec / 20.0).min(1.0)); // normalized 0-1
    }

    /// Check if currently streaming
    pub fn is_streaming(&self) -> bool {
        self.stream_status == FmStreamStatus::Streaming
    }

    /// Check if can send new prompt
    pub fn can_send(&self) -> bool {
        self.connection_status == FmConnectionStatus::Connected
            && !self.is_streaming()
            && !self.prompt_input.is_empty()
    }
}

impl Default for FmVizState {
    fn default() -> Self {
        Self::new()
    }
}
