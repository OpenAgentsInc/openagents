//! FM Bridge state for visualization

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

pub struct TranscriptMessage {
    pub role: &'static str,
    pub content: String,
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
    pub session_id: Option<String>,
    pub turn_count: u32,
    pub transcript: Vec<TranscriptMessage>,

    // Input
    pub prompt_input: String,
    pub cursor_pos: usize,
    pub selection: Option<(usize, usize)>, // (start, end)

    // Viz history
    pub token_history: Vec<f32>,
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

            session_id: None,
            turn_count: 0,
            transcript: Vec::new(),

            prompt_input: String::new(),
            cursor_pos: 0,
            selection: None,

            token_history: vec![0.0; 50],
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
            role: "USER",
            content: prompt.to_string(),
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
                role: "ASST",
                content: self.token_stream.clone(),
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
