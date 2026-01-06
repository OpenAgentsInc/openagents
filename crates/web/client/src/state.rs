#![allow(dead_code)]

use std::collections::VecDeque;

use editor::{EditorView, SyntaxLanguage};
use wasm_bindgen::prelude::JsValue;
use web_sys::WebSocket;
use wgpui::{
    Bounds, Component, Cursor, EventContext, EventResult, InputEvent, MarkdownDocument,
    MarkdownView, Point, StreamingMarkdown, TextInput,
};
use wgpui::components::hud::{DotsGrid, FrameAnimator};

use crate::hud::{HudContext, HudLayout, HudStreamHandle, HudUi, LandingLive};
use crate::nostr::{BazaarState, DvmDirectoryState, GlobalFeedState, Nip90State, NostrRelayHandle};
use crate::fs_access::FileEntry;
use crate::utils::copy_to_clipboard;
use crate::wallet::WalletUi;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum JobStatus {
    Working,
    Verifying,
    Paid,
}

#[derive(Clone)]
pub(crate) struct MarketJob {
    pub(crate) provider: &'static str,
    pub(crate) repo: &'static str,
    pub(crate) amount_sats: u32,
    pub(crate) status: JobStatus,
}

#[derive(Clone)]
pub(crate) struct MarketStats {
    pub(crate) jobs_today: u32,
    pub(crate) cleared_sats: u32,
    pub(crate) providers: u32,
}

impl Default for MarketStats {
    fn default() -> Self {
        Self {
            jobs_today: 1247,
            cleared_sats: 342000,
            providers: 89,
        }
    }
}

pub(crate) fn dummy_market_jobs() -> Vec<MarketJob> {
    vec![
        MarketJob { provider: "PatchGen", repo: "openagents/runtime#142", amount_sats: 4200, status: JobStatus::Paid },
        MarketJob { provider: "CodeReview", repo: "vercel/next.js#58921", amount_sats: 2800, status: JobStatus::Verifying },
        MarketJob { provider: "PatchGen", repo: "rust-lang/rust#12847", amount_sats: 6100, status: JobStatus::Paid },
        MarketJob { provider: "RepoIndex", repo: "facebook/react", amount_sats: 1400, status: JobStatus::Working },
        MarketJob { provider: "SandboxRun", repo: "tailwindlabs/ui#892", amount_sats: 450, status: JobStatus::Paid },
        MarketJob { provider: "PatchGen", repo: "tokio-rs/tokio#6234", amount_sats: 3800, status: JobStatus::Verifying },
    ]
}

#[derive(Clone, Default)]
pub(crate) struct UserInfo {
    pub(crate) github_username: Option<String>,
    pub(crate) nostr_npub: Option<String>,
}

#[derive(Clone)]
pub(crate) struct RepoInfo {
    pub(crate) full_name: String,
    pub(crate) description: Option<String>,
    pub(crate) private: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ClaudeToolRequest {
    pub(crate) session_id: String,
    pub(crate) tool: String,
    pub(crate) params: serde_json::Value,
}

pub(crate) struct ClaudeAgentState {
    pub(crate) status: String,
    pub(crate) repo: Option<String>,
    pub(crate) tunnel_session_id: Option<String>,
    pub(crate) tunnel_url: Option<String>,
    pub(crate) browser_url: Option<String>,
    pub(crate) connect_command: Option<String>,
    pub(crate) tunnel_connected: bool,
    pub(crate) ws: Option<WebSocket>,
    pub(crate) claude_session_id: Option<String>,
    pub(crate) pending_tool: Option<ClaudeToolRequest>,
    pub(crate) streaming_text: String,
}

impl ClaudeAgentState {
    pub(crate) fn reset(&mut self) {
        self.status = "idle".to_string();
        self.repo = None;
        self.tunnel_session_id = None;
        self.tunnel_url = None;
        self.browser_url = None;
        self.connect_command = None;
        self.tunnel_connected = false;
        self.ws = None;
        self.claude_session_id = None;
        self.pending_tool = None;
        self.streaming_text.clear();
    }
}

impl Default for ClaudeAgentState {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            repo: None,
            tunnel_session_id: None,
            tunnel_url: None,
            browser_url: None,
            connect_command: None,
            tunnel_connected: false,
            ws: None,
            claude_session_id: None,
            pending_tool: None,
            streaming_text: String::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum AppView {
    Landing,
    RepoSelector,
    RepoView,
    GfnPage,
    MlVizPage,
    GptOssPage,
    FmPage,
    FrlmPage,
    Y2026Page,
    BrbPage,
}

/// State for the GFN (Group Forming Networks) page
pub(crate) struct GfnState {
    /// Current node count (N), range 2-50
    pub(crate) node_count: u32,
    /// Slider track bounds for hit detection
    pub(crate) slider_bounds: Bounds,
    /// Whether the slider is being dragged
    pub(crate) slider_dragging: bool,
    /// Hover state for Metcalfe network diagram
    pub(crate) hover_metcalfe: bool,
    /// Hover state for Reed network diagram
    pub(crate) hover_reed: bool,
    /// CTA button bounds
    pub(crate) cta_bounds: Bounds,
    /// CTA button hover state
    pub(crate) cta_hovered: bool,
    /// Frame animator for the main card
    pub(crate) frame_animator: FrameAnimator,
    /// Whether the frame animation has started
    pub(crate) frame_started: bool,
    /// Scroll offset for the content
    pub(crate) scroll_offset: f32,
    /// Content bounds for scroll detection
    pub(crate) content_bounds: Bounds,
    /// Total content height for scroll calculation
    pub(crate) content_height: f32,
}

impl Default for GfnState {
    fn default() -> Self {
        Self {
            node_count: 8,
            slider_bounds: Bounds::ZERO,
            slider_dragging: false,
            hover_metcalfe: false,
            hover_reed: false,
            cta_bounds: Bounds::ZERO,
            cta_hovered: false,
            frame_animator: FrameAnimator::new(),
            frame_started: false,
            scroll_offset: 0.0,
            content_bounds: Bounds::ZERO,
            content_height: 0.0,
        }
    }
}

/// State for the 2026 page
pub(crate) struct Y2026State {
    /// Frame animator for the main card
    pub(crate) frame_animator: FrameAnimator,
    /// Whether the frame animation has started
    pub(crate) frame_started: bool,
    /// Link bounds for click detection (bounds, url)
    pub(crate) link_bounds: Vec<(Bounds, String)>,
    /// Whether any link is currently hovered
    pub(crate) link_hovered: bool,
}

impl Default for Y2026State {
    fn default() -> Self {
        Self {
            frame_animator: FrameAnimator::new(),
            frame_started: false,
            link_bounds: Vec::new(),
            link_hovered: false,
        }
    }
}

/// State for the FRLM (Fracking Apple Silicon) power comparison page
pub(crate) struct FrlmState {
    /// Frame animator for the main card
    pub(crate) frame_animator: FrameAnimator,
    /// Whether the frame animation has started
    pub(crate) frame_started: bool,
    /// Scroll offset for the content
    pub(crate) scroll_offset: f32,
    /// Content bounds for scroll detection
    pub(crate) content_bounds: Bounds,
    /// Total content height for scroll calculation
    pub(crate) content_height: f32,
    /// Which bar is currently hovered (0=DC, 1=Stargate, 2=Apple)
    pub(crate) bar_hover_index: Option<usize>,
    /// Bounds for each bar for hover detection
    pub(crate) bar_bounds: [Bounds; 3],
}

impl Default for FrlmState {
    fn default() -> Self {
        Self {
            frame_animator: FrameAnimator::new(),
            frame_started: false,
            scroll_offset: 0.0,
            content_bounds: Bounds::ZERO,
            content_height: 0.0,
            bar_hover_index: None,
            bar_bounds: [Bounds::ZERO; 3],
        }
    }
}

/// Connection status for FM Bridge
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) enum FmConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Stream status for FM Bridge token generation
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) enum FmStreamStatus {
    #[default]
    Idle,
    Streaming,
    Complete,
    Error,
}

/// Transcript message for FM Bridge sessions
#[derive(Clone, Debug)]
pub(crate) struct FmTranscriptMessage {
    pub(crate) role: String,
    pub(crate) content: String,
}

/// Tool info for FM Bridge
#[derive(Clone, Debug)]
pub(crate) struct FmToolInfo {
    pub(crate) name: String,
    pub(crate) description: String,
}

/// Tool invocation record
#[derive(Clone, Debug)]
pub(crate) struct FmToolInvocation {
    pub(crate) tool_name: String,
    pub(crate) timestamp_ms: u64,
    pub(crate) completed: bool,
    pub(crate) success: Option<bool>,
}

/// Log entry for FM Bridge events
#[derive(Clone, Debug)]
pub(crate) struct FmLogEntry {
    pub(crate) stage: String,
    pub(crate) status: String,
    pub(crate) detail: Option<String>,
    pub(crate) timestamp_ms: u64,
}

/// State for the FM Bridge (Apple Foundation Models) visualization page
pub(crate) struct FmVizState {
    pub(crate) frame_animator: FrameAnimator,
    pub(crate) frame_started: bool,
    pub(crate) scroll_offset: f32,
    pub(crate) content_bounds: Bounds,
    pub(crate) content_height: f32,

    // Bridge status
    pub(crate) connection_status: FmConnectionStatus,
    pub(crate) bridge_url: String,
    pub(crate) model_available: bool,
    pub(crate) ping_latency_ms: Option<u32>,

    // Prompt input
    pub(crate) prompt_input: TextInput,
    pub(crate) prompt_input_bounds: Bounds,
    pub(crate) run_button_bounds: Bounds,
    pub(crate) run_button_hovered: bool,
    pub(crate) input_event_ctx: EventContext,
    pub(crate) inputs_initialized: bool,

    // Token stream
    pub(crate) token_stream: String,
    pub(crate) stream_status: FmStreamStatus,
    pub(crate) tokens_per_sec: f32,
    pub(crate) ttft_ms: Option<u64>,
    pub(crate) token_count: usize,
    pub(crate) stream_start_ts_ms: Option<u64>,
    pub(crate) last_token_ts_ms: Option<u64>,

    // Session
    pub(crate) session_id: Option<String>,
    pub(crate) transcript: Vec<FmTranscriptMessage>,
    pub(crate) turn_count: u32,

    // Tools
    pub(crate) registered_tools: Vec<FmToolInfo>,
    pub(crate) tool_invocations: Vec<FmToolInvocation>,

    // Event log
    pub(crate) event_log: VecDeque<FmLogEntry>,

    // Demo mode
    pub(crate) demo_mode: bool,
    pub(crate) demo_tokens: Vec<&'static str>,
    pub(crate) demo_token_idx: usize,
    pub(crate) demo_last_tick: u64,
}

impl Default for FmVizState {
    fn default() -> Self {
        Self {
            frame_animator: FrameAnimator::new(),
            frame_started: false,
            scroll_offset: 0.0,
            content_bounds: Bounds::ZERO,
            content_height: 0.0,

            connection_status: FmConnectionStatus::Disconnected,
            bridge_url: "http://localhost:11435".to_string(),
            model_available: false,
            ping_latency_ms: None,

            prompt_input: TextInput::new(),
            prompt_input_bounds: Bounds::ZERO,
            run_button_bounds: Bounds::ZERO,
            run_button_hovered: false,
            input_event_ctx: EventContext::new(),
            inputs_initialized: false,

            token_stream: String::new(),
            stream_status: FmStreamStatus::Idle,
            tokens_per_sec: 0.0,
            ttft_ms: None,
            token_count: 0,
            stream_start_ts_ms: None,
            last_token_ts_ms: None,

            session_id: None,
            transcript: Vec::new(),
            turn_count: 0,

            registered_tools: Vec::new(),
            tool_invocations: Vec::new(),

            event_log: VecDeque::new(),

            demo_mode: false,
            demo_tokens: vec![
                "Apple", " Foundation", " Models", " provide", " on", "-device",
                " intelligence", " for", " your", " apps", ".", " With", " the",
                " Foundation", " Models", " framework", ",", " you", " can",
                " integrate", " powerful", " language", " models", " directly",
                " into", " your", " applications", ".", "\n\n", "The", " model",
                " runs", " entirely", " on", " device", ",", " ensuring", " privacy",
                " and", " low", " latency", ".", " No", " internet", " connection",
                " required", ".", "\n\n", "Features", ":", "\n", "-", " Text",
                " generation", "\n", "-", " Summarization", "\n", "-", " Tool",
                " calling", "\n", "-", " Structured", " output", " (", "JSON", ")",
            ],
            demo_token_idx: 0,
            demo_last_tick: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MlTokenCandidate {
    pub(crate) token_id: u32,
    pub(crate) token_text: String,
    pub(crate) probability: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct LayerActivity {
    pub(crate) layer: usize,
    pub(crate) attention_norm: f32,
    pub(crate) mlp_norm: f32,
    pub(crate) output_norm: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct CacheInfo {
    pub(crate) layer: usize,
    pub(crate) seq_len: usize,
    pub(crate) max_len: usize,
    pub(crate) offset: usize,
    pub(crate) memory_bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryUsage {
    pub(crate) gpu_allocated: usize,
    pub(crate) cache_total: usize,
    pub(crate) activations: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct TensorInfo {
    pub(crate) name: String,
    pub(crate) bytes: usize,
    pub(crate) kind: String,
}

#[derive(Clone)]
pub(crate) struct GpuContext {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
}

impl GpuContext {
    pub(crate) fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self { device, queue }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GptOssStageStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

impl Default for GptOssStageStatus {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GptOssStage {
    pub(crate) name: String,
    pub(crate) status: GptOssStageStatus,
    pub(crate) detail: Option<String>,
    pub(crate) bytes: Option<u64>,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) step: Option<usize>,
    pub(crate) total_steps: Option<usize>,
    pub(crate) ts_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct GptOssLogEntry {
    pub(crate) ts_ms: Option<u64>,
    pub(crate) message: String,
    pub(crate) status: GptOssStageStatus,
}

/// State for the GPT-OSS model loading/inference visualization page
pub(crate) struct GptOssVizState {
    pub(crate) frame_animator: FrameAnimator,
    pub(crate) frame_started: bool,
    pub(crate) scroll_offset: f32,
    pub(crate) content_bounds: Bounds,
    pub(crate) content_height: f32,
    pub(crate) start_button_bounds: Bounds,
    pub(crate) start_button_hovered: bool,
    pub(crate) file_button_bounds: Bounds,
    pub(crate) file_button_hovered: bool,
    pub(crate) copy_button_bounds: Bounds,
    pub(crate) copy_button_hovered: bool,
    pub(crate) drop_active: bool,
    pub(crate) gguf_input_bounds: Bounds,
    pub(crate) prompt_input_bounds: Bounds,
    pub(crate) layers_input_bounds: Bounds,
    pub(crate) max_kv_input_bounds: Bounds,
    pub(crate) max_new_input_bounds: Bounds,
    pub(crate) sample_input_bounds: Bounds,
    pub(crate) temp_input_bounds: Bounds,
    pub(crate) top_k_input_bounds: Bounds,
    pub(crate) top_p_input_bounds: Bounds,
    pub(crate) gguf_input: TextInput,
    pub(crate) prompt_input: TextInput,
    pub(crate) layers_input: TextInput,
    pub(crate) max_kv_input: TextInput,
    pub(crate) max_new_input: TextInput,
    pub(crate) sample_input: TextInput,
    pub(crate) temp_input: TextInput,
    pub(crate) top_k_input: TextInput,
    pub(crate) top_p_input: TextInput,
    pub(crate) gguf_file: Option<web_sys::File>,
    pub(crate) gguf_file_label: Option<String>,
    pub(crate) input_event_ctx: EventContext,
    pub(crate) inputs_initialized: bool,
    pub(crate) load_active: bool,
    pub(crate) load_error: Option<String>,
    pub(crate) inference_error: Option<String>,
    pub(crate) load_url: Option<String>,
    pub(crate) load_progress: Option<f32>,
    pub(crate) load_stages: Vec<GptOssStage>,
    pub(crate) inference_stages: Vec<GptOssStage>,
    pub(crate) events: VecDeque<GptOssLogEntry>,
    pub(crate) token_stream: String,
    pub(crate) last_token_id: Option<u32>,
    pub(crate) top_k: Vec<MlTokenCandidate>,
    pub(crate) probability_history: VecDeque<Vec<MlTokenCandidate>>,
    pub(crate) tokens_per_sec: Option<f32>,
    pub(crate) entropy: Option<f32>,
    pub(crate) entropy_history: VecDeque<f32>,
    pub(crate) memory_usage: Option<MemoryUsage>,
    pub(crate) gpu_limits: Option<String>,
    pub(crate) token_limits: Option<String>,
    pub(crate) cache_status: Vec<CacheInfo>,
    pub(crate) resident_tensors: Vec<TensorInfo>,
    pub(crate) recent_tensors: VecDeque<String>,
    pub(crate) attention_weights: Option<Vec<Vec<f32>>>,
    pub(crate) attention_layer: usize,
    pub(crate) attention_head: usize,
    pub(crate) attention_selected_layer: usize,
    pub(crate) attention_selected_head: usize,
    pub(crate) layer_activations: Vec<LayerActivity>,
    pub(crate) max_layers: usize,
    pub(crate) max_heads: usize,
    pub(crate) layer_slider_bounds: Bounds,
    pub(crate) head_slider_bounds: Bounds,
    pub(crate) layer_slider_dragging: bool,
    pub(crate) head_slider_dragging: bool,
    pub(crate) attention_mode: Option<String>,
    pub(crate) moe_mode: Option<String>,
    pub(crate) sampling_mode: Option<String>,
    pub(crate) cpu_fallback: Option<String>,
    pub(crate) active_layers: Option<usize>,
    pub(crate) current_stage: Option<String>,
    pub(crate) last_token_ts_ms: Option<u64>,
    pub(crate) start_ts_ms: Option<u64>,
}

impl Default for GptOssVizState {
    fn default() -> Self {
        Self {
            frame_animator: FrameAnimator::new(),
            frame_started: false,
            scroll_offset: 0.0,
            content_bounds: Bounds::ZERO,
            content_height: 0.0,
            start_button_bounds: Bounds::ZERO,
            start_button_hovered: false,
            file_button_bounds: Bounds::ZERO,
            file_button_hovered: false,
            copy_button_bounds: Bounds::ZERO,
            copy_button_hovered: false,
            drop_active: false,
            gguf_input_bounds: Bounds::ZERO,
            prompt_input_bounds: Bounds::ZERO,
            layers_input_bounds: Bounds::ZERO,
            max_kv_input_bounds: Bounds::ZERO,
            max_new_input_bounds: Bounds::ZERO,
            sample_input_bounds: Bounds::ZERO,
            temp_input_bounds: Bounds::ZERO,
            top_k_input_bounds: Bounds::ZERO,
            top_p_input_bounds: Bounds::ZERO,
            gguf_input: TextInput::new()
                .placeholder("pylon://localhost:9899 or https://...gguf")
                .font_size(10.0)
                .padding(6.0, 4.0),
            prompt_input: TextInput::new()
                .placeholder("Enter a prompt")
                .font_size(10.0)
                .padding(6.0, 4.0),
            layers_input: TextInput::new()
                .placeholder("all")
                .font_size(10.0)
                .padding(6.0, 4.0),
            max_kv_input: TextInput::new()
                .placeholder("32")
                .font_size(10.0)
                .padding(6.0, 4.0),
            max_new_input: TextInput::new()
                .placeholder("8")
                .font_size(10.0)
                .padding(6.0, 4.0),
            sample_input: TextInput::new()
                .placeholder("on")
                .font_size(10.0)
                .padding(6.0, 4.0),
            temp_input: TextInput::new()
                .placeholder("1.0")
                .font_size(10.0)
                .padding(6.0, 4.0),
            top_k_input: TextInput::new()
                .placeholder("40")
                .font_size(10.0)
                .padding(6.0, 4.0),
            top_p_input: TextInput::new()
                .placeholder("1.0")
                .font_size(10.0)
                .padding(6.0, 4.0),
            gguf_file: None,
            gguf_file_label: None,
            input_event_ctx: EventContext::new(),
            inputs_initialized: false,
            load_active: false,
            load_error: None,
            inference_error: None,
            load_url: None,
            load_progress: None,
            load_stages: Vec::new(),
            inference_stages: Vec::new(),
            events: VecDeque::with_capacity(120),
            token_stream: String::new(),
            last_token_id: None,
            top_k: Vec::new(),
            probability_history: VecDeque::with_capacity(18),
            tokens_per_sec: None,
            entropy: None,
            entropy_history: VecDeque::with_capacity(32),
            memory_usage: None,
            gpu_limits: None,
            token_limits: None,
            cache_status: Vec::new(),
            resident_tensors: Vec::new(),
            recent_tensors: VecDeque::with_capacity(12),
            attention_weights: None,
            attention_layer: 0,
            attention_head: 0,
            attention_selected_layer: 0,
            attention_selected_head: 0,
            layer_activations: Vec::new(),
            max_layers: 1,
            max_heads: 1,
            layer_slider_bounds: Bounds::ZERO,
            head_slider_bounds: Bounds::ZERO,
            layer_slider_dragging: false,
            head_slider_dragging: false,
            attention_mode: None,
            moe_mode: None,
            sampling_mode: None,
            cpu_fallback: None,
            active_layers: None,
            current_stage: None,
            last_token_ts_ms: None,
            start_ts_ms: None,
        }
    }
}

impl GptOssVizState {
    pub(crate) fn handle_event(&mut self, event: &InputEvent) -> EventResult {
        let mut handled = EventResult::Ignored;
        handled = merge_event_result(
            handled,
            self.gguf_input
                .event(event, self.gguf_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.prompt_input
                .event(event, self.prompt_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.layers_input
                .event(event, self.layers_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.max_kv_input
                .event(event, self.max_kv_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.max_new_input
                .event(event, self.max_new_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.sample_input
                .event(event, self.sample_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.temp_input
                .event(event, self.temp_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.top_k_input
                .event(event, self.top_k_input_bounds, &mut self.input_event_ctx),
        );
        handled = merge_event_result(
            handled,
            self.top_p_input
                .event(event, self.top_p_input_bounds, &mut self.input_event_ctx),
        );
        handled
    }

    pub(crate) fn input_focused(&self) -> bool {
        self.gguf_input.is_focused()
            || self.prompt_input.is_focused()
            || self.layers_input.is_focused()
            || self.max_kv_input.is_focused()
            || self.max_new_input.is_focused()
            || self.sample_input.is_focused()
            || self.temp_input.is_focused()
            || self.top_k_input.is_focused()
            || self.top_p_input.is_focused()
    }

    pub(crate) fn paste_text(&mut self, text: &str) -> bool {
        if self.gguf_input.is_focused() {
            self.gguf_input.insert_text(text);
            return true;
        }
        if self.prompt_input.is_focused() {
            self.prompt_input.insert_text(text);
            return true;
        }
        if self.layers_input.is_focused() {
            self.layers_input.insert_text(text);
            return true;
        }
        if self.max_kv_input.is_focused() {
            self.max_kv_input.insert_text(text);
            return true;
        }
        if self.max_new_input.is_focused() {
            self.max_new_input.insert_text(text);
            return true;
        }
        if self.sample_input.is_focused() {
            self.sample_input.insert_text(text);
            return true;
        }
        if self.temp_input.is_focused() {
            self.temp_input.insert_text(text);
            return true;
        }
        if self.top_k_input.is_focused() {
            self.top_k_input.insert_text(text);
            return true;
        }
        if self.top_p_input.is_focused() {
            self.top_p_input.insert_text(text);
            return true;
        }
        false
    }

    pub(crate) fn build_debug_report(&self) -> String {
        let mut out = String::new();
        out.push_str("GPT-OSS LOGS\n");
        out.push_str("----------------\n");
        let gguf_label = self
            .load_url
            .as_deref()
            .or(self.gguf_file_label.as_deref())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| self.gguf_input.get_value().trim());
        if !gguf_label.is_empty() {
            out.push_str("gguf: ");
            out.push_str(gguf_label);
            out.push('\n');
        }
        let prompt = self.prompt_input.get_value();
        if !prompt.trim().is_empty() {
            out.push_str("prompt: ");
            out.push_str(prompt.trim());
            out.push('\n');
        }
        out.push_str("inputs: ");
        out.push_str(&format!(
            "layers={} max_kv={} max_new={} sample={} temp={} top_k={} top_p={}\n",
            self.layers_input.get_value().trim(),
            self.max_kv_input.get_value().trim(),
            self.max_new_input.get_value().trim(),
            self.sample_input.get_value().trim(),
            self.temp_input.get_value().trim(),
            self.top_k_input.get_value().trim(),
            self.top_p_input.get_value().trim(),
        ));
        out.push_str(&format!(
            "status: load_active={} current_stage={}\n",
            self.load_active,
            self.current_stage.as_deref().unwrap_or("-")
        ));
        if let Some(err) = &self.load_error {
            out.push_str("load_error: ");
            out.push_str(err);
            out.push('\n');
        }
        if let Some(err) = &self.inference_error {
            out.push_str("inference_error: ");
            out.push_str(err);
            out.push('\n');
        }
        if let Some(token_limits) = &self.token_limits {
            out.push_str("token_limits: ");
            out.push_str(token_limits);
            out.push('\n');
        }
        if let Some(gpu_limits) = &self.gpu_limits {
            out.push_str("gpu_limits: ");
            out.push_str(gpu_limits);
            out.push('\n');
        }
        if let Some(memory) = &self.memory_usage {
            out.push_str("memory: ");
            out.push_str(&format!(
                "gpu_allocated={} cache_total={} activations={}\n",
                format_bytes_u64(memory.gpu_allocated as u64),
                format_bytes_u64(memory.cache_total as u64),
                format_bytes_u64(memory.activations as u64),
            ));
        }
        if let Some(entropy) = self.entropy {
            out.push_str(&format!("entropy: {:.4}\n", entropy));
        }
        if let Some(tokens_per_sec) = self.tokens_per_sec {
            out.push_str(&format!("tokens_per_sec: {:.2}\n", tokens_per_sec));
        }
        if let Some(token_id) = self.last_token_id {
            out.push_str(&format!("last_token_id: {token_id}\n"));
        }
        if !self.token_stream.is_empty() {
            out.push_str("token_stream:\n");
            out.push_str(self.token_stream.trim_end());
            out.push('\n');
        }
        if !self.top_k.is_empty() {
            out.push_str("top_k:\n");
            for cand in &self.top_k {
                out.push_str(&format!(
                    "- id={} prob={:.5} text={}\n",
                    cand.token_id,
                    cand.probability,
                    sanitize_line(&cand.token_text)
                ));
            }
        }
        if !self.load_stages.is_empty() {
            out.push_str("load_stages:\n");
            for stage in &self.load_stages {
                out.push_str(&format_stage_line(stage));
            }
        }
        if !self.inference_stages.is_empty() {
            out.push_str("inference_stages:\n");
            for stage in &self.inference_stages {
                out.push_str(&format_stage_line(stage));
            }
        }
        if !self.events.is_empty() {
            out.push_str("events:\n");
            for entry in &self.events {
                let ts = entry.ts_ms.unwrap_or(0);
                out.push_str(&format!(
                    "- ts={} status={} msg={}\n",
                    ts,
                    stage_status_label(entry.status),
                    sanitize_line(&entry.message)
                ));
            }
        }
        if !self.cache_status.is_empty() {
            out.push_str("cache_status:\n");
            for cache in &self.cache_status {
                out.push_str(&format!(
                    "- layer={} seq_len={} max_len={} offset={} bytes={}\n",
                    cache.layer,
                    cache.seq_len,
                    cache.max_len,
                    cache.offset,
                    format_bytes_u64(cache.memory_bytes as u64)
                ));
            }
        }
        if !self.layer_activations.is_empty() {
            out.push_str("layer_activations:\n");
            for act in &self.layer_activations {
                out.push_str(&format!(
                    "- layer={} attn_norm={:.4} mlp_norm={:.4} out_norm={:.4}\n",
                    act.layer, act.attention_norm, act.mlp_norm, act.output_norm
                ));
            }
        }
        if !self.recent_tensors.is_empty() {
            out.push_str("recent_tensors:\n");
            for name in &self.recent_tensors {
                out.push_str("- ");
                out.push_str(name);
                out.push('\n');
            }
        }
        if !self.resident_tensors.is_empty() {
            out.push_str("resident_tensors:\n");
            for tensor in &self.resident_tensors {
                out.push_str(&format!(
                    "- {} bytes={} kind={}\n",
                    tensor.name,
                    format_bytes_u64(tensor.bytes as u64),
                    tensor.kind
                ));
            }
        }
        out
    }
}

fn stage_status_label(status: GptOssStageStatus) -> &'static str {
    match status {
        GptOssStageStatus::Idle => "IDLE",
        GptOssStageStatus::Running => "RUN",
        GptOssStageStatus::Completed => "OK",
        GptOssStageStatus::Failed => "FAIL",
    }
}

fn format_stage_line(stage: &GptOssStage) -> String {
    let mut line = format!("- [{}] {}", stage_status_label(stage.status), stage.name);
    if let Some(detail) = stage.detail.as_ref() {
        line.push_str(" detail=");
        line.push_str(&sanitize_line(detail));
    }
    if let (Some(bytes), Some(total)) = (stage.bytes, stage.total_bytes) {
        line.push_str(&format!(
            " bytes={}/{}",
            format_bytes_u64(bytes),
            format_bytes_u64(total)
        ));
    } else if let Some(bytes) = stage.bytes {
        line.push_str(&format!(" bytes={}", format_bytes_u64(bytes)));
    }
    if let (Some(step), Some(total)) = (stage.step, stage.total_steps) {
        line.push_str(&format!(" step={}/{}", step, total));
    }
    if let Some(ts) = stage.ts_ms {
        line.push_str(&format!(" ts={ts}"));
    }
    line.push('\n');
    line
}

fn sanitize_line(value: &str) -> String {
    value.replace('\n', " | ").replace('\r', " ")
}

fn format_bytes_u64(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1}GB", bytes as f32 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f32 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f32 / 1_000.0)
    } else {
        format!("{bytes}B")
    }
}

fn merge_event_result(lhs: EventResult, rhs: EventResult) -> EventResult {
    match (lhs, rhs) {
        (EventResult::Handled, _) | (_, EventResult::Handled) => EventResult::Handled,
        _ => EventResult::Ignored,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateStatus {
    Idle,
    Running,
    Passed,
    Failed,
}

impl Default for GateStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// State for the ML inference visualization page
pub(crate) struct MlVizState {
    pub(crate) frame_animator: FrameAnimator,
    pub(crate) frame_started: bool,
    pub(crate) scroll_offset: f32,
    pub(crate) content_bounds: Bounds,
    pub(crate) content_height: f32,
    pub(crate) layer_slider_bounds: Bounds,
    pub(crate) head_slider_bounds: Bounds,
    pub(crate) layer_slider_dragging: bool,
    pub(crate) head_slider_dragging: bool,
    pub(crate) selected_layer: usize,
    pub(crate) selected_head: usize,
    pub(crate) max_layers: usize,
    pub(crate) max_heads: usize,
    pub(crate) token_stream: String,
    pub(crate) last_token_id: Option<u32>,
    pub(crate) top_k: Vec<MlTokenCandidate>,
    pub(crate) tokens_per_sec: Option<f32>,
    pub(crate) entropy: Option<f32>,
    pub(crate) entropy_history: VecDeque<f32>,
    pub(crate) probability_history: VecDeque<Vec<MlTokenCandidate>>,
    pub(crate) attention_weights: Option<Vec<Vec<f32>>>,
    pub(crate) attention_layer: usize,
    pub(crate) attention_head: usize,
    pub(crate) layer_activations: Vec<LayerActivity>,
    pub(crate) cache_status: Vec<CacheInfo>,
    pub(crate) memory_usage: Option<MemoryUsage>,
    pub(crate) gate_status: GateStatus,
    pub(crate) gate_started: bool,
    pub(crate) gate_message: Option<String>,
    pub(crate) gate_error: Option<String>,
    pub(crate) gate_source: Option<String>,
    pub(crate) gate_tensor: Option<String>,
    pub(crate) gate_k: Option<usize>,
    pub(crate) gate_n: Option<usize>,
    pub(crate) gate_bytes: Option<usize>,
    pub(crate) gate_max_abs: Option<f32>,
    pub(crate) gate_mean_abs: Option<f32>,
}

impl Default for MlVizState {
    fn default() -> Self {
        Self {
            frame_animator: FrameAnimator::new(),
            frame_started: false,
            scroll_offset: 0.0,
            content_bounds: Bounds::ZERO,
            content_height: 0.0,
            layer_slider_bounds: Bounds::ZERO,
            head_slider_bounds: Bounds::ZERO,
            layer_slider_dragging: false,
            head_slider_dragging: false,
            selected_layer: 0,
            selected_head: 0,
            max_layers: 1,
            max_heads: 1,
            token_stream: String::new(),
            last_token_id: None,
            top_k: Vec::new(),
            tokens_per_sec: None,
            entropy: None,
            entropy_history: VecDeque::with_capacity(64),
            probability_history: VecDeque::with_capacity(32),
            attention_weights: None,
            attention_layer: 0,
            attention_head: 0,
            layer_activations: Vec::new(),
            cache_status: Vec::new(),
            memory_usage: None,
            gate_status: GateStatus::Idle,
            gate_started: false,
            gate_message: None,
            gate_error: None,
            gate_source: None,
            gate_tensor: None,
            gate_k: None,
            gate_n: None,
            gate_bytes: None,
            gate_max_abs: None,
            gate_mean_abs: None,
        }
    }
}

pub(crate) struct MarkdownDemo {
    pub(crate) streaming: StreamingMarkdown,
    pub(crate) view: MarkdownView,
    pub(crate) tokens: VecDeque<String>,
    pub(crate) last_token_frame: u64,
    pub(crate) frame_count: u64,
    pub(crate) source: String,
    pub(crate) bounds: Bounds,
    pub(crate) events: EventContext,
}

impl MarkdownDemo {
    pub(crate) fn new() -> Self {
        let source = demo_markdown_source();
        let tokens = tokenize_markdown(&source);
        let view = MarkdownView::new(MarkdownDocument::new())
            .copy_button_on_hover(true)
            .on_copy(copy_to_clipboard);

        Self {
            streaming: StreamingMarkdown::new(),
            view,
            tokens,
            last_token_frame: 0,
            frame_count: 0,
            source,
            bounds: Bounds::ZERO,
            events: EventContext::new(),
        }
    }

    pub(crate) fn tick(&mut self) {
        self.frame_count += 1;

        let frames_since_token = self.frame_count - self.last_token_frame;
        if frames_since_token >= 2 && !self.tokens.is_empty() {
            if let Some(token) = self.tokens.pop_front() {
                self.streaming.append(&token);
                self.last_token_frame = self.frame_count;
            }
        }

        if self.tokens.is_empty() && self.streaming.has_pending() {
            self.streaming.complete();
        }

        self.streaming.tick();
    }

    pub(crate) fn handle_event(&mut self, event: InputEvent) -> EventResult {
        if self.bounds.size.width <= 0.0 || self.bounds.size.height <= 0.0 {
            return EventResult::Ignored;
        }
        self.view.event(&event, self.bounds, &mut self.events)
    }

    pub(crate) fn cursor(&self) -> Cursor {
        if self.bounds.size.width <= 0.0 || self.bounds.size.height <= 0.0 {
            Cursor::Default
        } else {
            self.view.cursor()
        }
    }

    pub(crate) fn clear_hover(&mut self) {
        self.view.clear_hover();
    }
}

pub(crate) struct EditorBuffer {
    pub(crate) name: String,
    pub(crate) path: Option<String>,
    pub(crate) handle: Option<JsValue>,
    pub(crate) view: EditorView,
    pub(crate) events: EventContext,
}

impl EditorBuffer {
    fn new(name: String, path: Option<String>, handle: Option<JsValue>, text: &str) -> Self {
        let mut view = EditorView::from_text(text).on_copy(copy_to_clipboard);
        if let Some(language) = SyntaxLanguage::from_path(path.as_deref().unwrap_or(&name)) {
            view.set_language(Some(language));
        }
        Self {
            name,
            path,
            handle,
            view,
            events: EventContext::new(),
        }
    }
}

pub(crate) struct EditorPane {
    pub(crate) bounds: Bounds,
    pub(crate) editor_bounds: Bounds,
    pub(crate) tab_bounds: Vec<Bounds>,
    pub(crate) active_buffer: Option<usize>,
}

impl EditorPane {
    fn new() -> Self {
        Self {
            bounds: Bounds::ZERO,
            editor_bounds: Bounds::ZERO,
            tab_bounds: Vec::new(),
            active_buffer: None,
        }
    }
}

pub(crate) struct EditorWorkspace {
    pub(crate) buffers: Vec<EditorBuffer>,
    pub(crate) panes: Vec<EditorPane>,
    pub(crate) active_pane: usize,
    pub(crate) split: bool,
    pub(crate) bounds: Bounds,
    pub(crate) buffer_list_bounds: Bounds,
    pub(crate) buffer_row_bounds: Vec<Bounds>,
    pub(crate) split_toggle_bounds: Bounds,
    pub(crate) new_buffer_bounds: Bounds,
    pub(crate) hovered_buffer_idx: Option<usize>,
    pub(crate) hovered_tab: Option<(usize, usize)>,
    pub(crate) hovered_split_toggle: bool,
    pub(crate) hovered_new_buffer: bool,
    untitled_counter: usize,
}

impl EditorWorkspace {
    pub(crate) fn new() -> Self {
        let source = demo_editor_source();
        let mut buffers = Vec::new();
        buffers.push(EditorBuffer::new(
            "views.rs".to_string(),
            None,
            None,
            &source,
        ));

        let mut panes = vec![EditorPane::new(), EditorPane::new()];
        panes[0].active_buffer = Some(0);

        Self {
            buffers,
            panes,
            active_pane: 0,
            split: false,
            bounds: Bounds::ZERO,
            buffer_list_bounds: Bounds::ZERO,
            buffer_row_bounds: Vec::new(),
            split_toggle_bounds: Bounds::ZERO,
            new_buffer_bounds: Bounds::ZERO,
            hovered_buffer_idx: None,
            hovered_tab: None,
            hovered_split_toggle: false,
            hovered_new_buffer: false,
            untitled_counter: 1,
        }
    }

    pub(crate) fn add_scratch_buffer(&mut self) -> usize {
        let name = format!("Untitled {}", self.untitled_counter);
        self.untitled_counter += 1;
        let idx = self.create_buffer(name, None, None, "");
        self.set_active_buffer(self.active_pane, idx);
        idx
    }

    pub(crate) fn open_file(&mut self, path: String, handle: JsValue, contents: String) -> usize {
        if let Some(idx) = self
            .buffers
            .iter()
            .position(|buffer| buffer.path.as_deref() == Some(path.as_str()))
        {
            let buffer = &mut self.buffers[idx];
            buffer.view.set_text(&contents);
            buffer.handle = Some(handle);
            buffer.path = Some(path);
            if let Some(language) = SyntaxLanguage::from_path(buffer.path.as_deref().unwrap_or("")) {
                buffer.view.set_language(Some(language));
            }
            self.set_active_buffer(self.active_pane, idx);
            return idx;
        }

        let name = path
            .split('/')
            .last()
            .filter(|part| !part.is_empty())
            .unwrap_or(&path)
            .to_string();
        let idx = self.create_buffer(name, Some(path), Some(handle), &contents);
        self.set_active_buffer(self.active_pane, idx);
        idx
    }

    pub(crate) fn toggle_split(&mut self) {
        if self.split {
            self.split = false;
            self.panes[1].active_buffer = None;
            self.active_pane = 0;
            return;
        }

        self.split = true;
        if self.panes[1].active_buffer.is_none() {
            let fallback = (0..self.buffers.len())
                .find(|idx| Some(*idx) != self.panes[0].active_buffer)
                .unwrap_or_else(|| {
                    let name = format!("Untitled {}", self.untitled_counter);
                    self.untitled_counter += 1;
                    self.create_buffer(name, None, None, "")
                });
            self.panes[1].active_buffer = Some(fallback);
        }
    }

    pub(crate) fn active_buffer_path(&self) -> Option<&str> {
        let idx = self.active_buffer_idx()?;
        self.buffers[idx].path.as_deref()
    }

    pub(crate) fn active_buffer_handle(&self) -> Option<JsValue> {
        let idx = self.active_buffer_idx()?;
        self.buffers[idx].handle.clone()
    }

    pub(crate) fn active_buffer_text(&self) -> Option<String> {
        let idx = self.active_buffer_idx()?;
        Some(self.buffers[idx].view.editor().text())
    }

    pub(crate) fn cursor(&self) -> Cursor {
        let pane_count = if self.split { 2 } else { 1 };
        for pane_idx in 0..pane_count {
            if let Some(buffer_idx) = self.panes[pane_idx].active_buffer {
                if matches!(self.buffers[buffer_idx].view.cursor(), Cursor::Text) {
                    return Cursor::Text;
                }
            }
        }
        Cursor::Default
    }

    pub(crate) fn is_focused(&self) -> bool {
        self.active_buffer_idx()
            .and_then(|idx| self.buffers.get(idx))
            .map(|buffer| buffer.view.is_focused())
            .unwrap_or(false)
    }

    pub(crate) fn paste_text(&mut self, text: &str) {
        if let Some(idx) = self.active_buffer_idx() {
            self.buffers[idx].view.paste_text(text);
        }
    }

    pub(crate) fn composition_start(&mut self, text: &str) {
        if let Some(idx) = self.active_buffer_idx() {
            self.buffers[idx].view.composition_start(text);
        }
    }

    pub(crate) fn composition_update(&mut self, text: &str) {
        if let Some(idx) = self.active_buffer_idx() {
            self.buffers[idx].view.composition_update(text);
        }
    }

    pub(crate) fn composition_end(&mut self, text: &str) {
        if let Some(idx) = self.active_buffer_idx() {
            self.buffers[idx].view.composition_end(text);
        }
    }

    pub(crate) fn clear_hover(&mut self) {
        for buffer in &mut self.buffers {
            buffer.view.clear_hover();
        }
    }

    pub(crate) fn update_hover(&mut self, point: Point) {
        self.hovered_buffer_idx = None;
        for (idx, bounds) in self.buffer_row_bounds.iter().enumerate() {
            if bounds.contains(point) {
                self.hovered_buffer_idx = Some(idx);
                break;
            }
        }

        self.hovered_tab = None;
        let pane_count = if self.split { 2 } else { 1 };
        for pane_idx in 0..pane_count {
            for (buffer_idx, bounds) in self.panes[pane_idx].tab_bounds.iter().enumerate() {
                if bounds.contains(point) {
                    self.hovered_tab = Some((pane_idx, buffer_idx));
                    break;
                }
            }
            if self.hovered_tab.is_some() {
                break;
            }
        }

        self.hovered_split_toggle = self.split_toggle_bounds.contains(point);
        self.hovered_new_buffer = self.new_buffer_bounds.contains(point);

        let _ = self.handle_mouse_event(InputEvent::MouseMove { x: point.x, y: point.y });
    }

    pub(crate) fn handle_mouse_event(&mut self, event: InputEvent) -> EventResult {
        match event {
            InputEvent::MouseMove { .. } => {
                let pane_count = if self.split { 2 } else { 1 };
                for pane_idx in 0..pane_count {
                    let Some(buffer_idx) = self.panes[pane_idx].active_buffer else {
                        continue;
                    };
                    let bounds = self.panes[pane_idx].editor_bounds;
                    let buffer = &mut self.buffers[buffer_idx];
                    let _ = buffer.view.event(&event, bounds, &mut buffer.events);
                }
                EventResult::Handled
            }
            InputEvent::MouseDown { x, y, .. } => {
                let point = Point::new(x, y);
                let pane_count = if self.split { 2 } else { 1 };
                let mut hit_pane = None;
                for pane_idx in 0..pane_count {
                    let bounds = self.panes[pane_idx].editor_bounds;
                    if bounds.contains(point) {
                        hit_pane = Some(pane_idx);
                        break;
                    }
                }
                if let Some(pane_idx) = hit_pane {
                    self.active_pane = pane_idx;
                }
                for pane_idx in 0..pane_count {
                    if Some(pane_idx) == hit_pane {
                        continue;
                    }
                    if let Some(buffer_idx) = self.panes[pane_idx].active_buffer {
                        let bounds = self.panes[pane_idx].editor_bounds;
                        let buffer = &mut self.buffers[buffer_idx];
                        let _ = buffer.view.event(&event, bounds, &mut buffer.events);
                    }
                }
                if let Some(pane_idx) = hit_pane {
                    if let Some(buffer_idx) = self.panes[pane_idx].active_buffer {
                        let bounds = self.panes[pane_idx].editor_bounds;
                        let buffer = &mut self.buffers[buffer_idx];
                        return buffer.view.event(&event, bounds, &mut buffer.events);
                    }
                }
                EventResult::Ignored
            }
            InputEvent::MouseUp { .. } => {
                if let Some(buffer_idx) = self.active_buffer_idx() {
                    let bounds = self.panes[self.active_pane].editor_bounds;
                    let buffer = &mut self.buffers[buffer_idx];
                    return buffer.view.event(&event, bounds, &mut buffer.events);
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }

    pub(crate) fn handle_scroll_at(&mut self, point: Point, event: InputEvent) -> EventResult {
        let pane_count = if self.split { 2 } else { 1 };
        for pane_idx in 0..pane_count {
            let bounds = self.panes[pane_idx].editor_bounds;
            if bounds.contains(point) {
                if let Some(buffer_idx) = self.panes[pane_idx].active_buffer {
                    let buffer = &mut self.buffers[buffer_idx];
                    return buffer.view.event(&event, bounds, &mut buffer.events);
                }
            }
        }
        EventResult::Ignored
    }

    pub(crate) fn handle_key_event(&mut self, event: InputEvent) -> EventResult {
        let Some(buffer_idx) = self.active_buffer_idx() else {
            return EventResult::Ignored;
        };
        let bounds = self.panes[self.active_pane].editor_bounds;
        let buffer = &mut self.buffers[buffer_idx];
        buffer.view.event(&event, bounds, &mut buffer.events)
    }

    pub(crate) fn set_active_buffer(&mut self, pane_idx: usize, buffer_idx: usize) {
        if pane_idx >= self.panes.len() || buffer_idx >= self.buffers.len() {
            return;
        }
        let other_idx = if pane_idx == 0 { 1 } else { 0 };
        let previous = self.panes[pane_idx].active_buffer;
        if other_idx < self.panes.len() && self.panes[other_idx].active_buffer == Some(buffer_idx) {
            self.panes[other_idx].active_buffer = previous;
        }
        self.panes[pane_idx].active_buffer = Some(buffer_idx);
        self.active_pane = pane_idx;
    }

    fn active_buffer_idx(&self) -> Option<usize> {
        self.panes.get(self.active_pane)?.active_buffer
    }

    fn create_buffer(
        &mut self,
        name: String,
        path: Option<String>,
        handle: Option<JsValue>,
        text: &str,
    ) -> usize {
        let buffer = EditorBuffer::new(name, path, handle, text);
        self.buffers.push(buffer);
        self.buffers.len() - 1
    }
}

pub(crate) struct AppState {
    pub(crate) mouse_pos: Point,
    pub(crate) button_hovered: bool,
    pub(crate) button_bounds: Bounds,
    pub(crate) landing_issue_bounds: Bounds,
    pub(crate) landing_issue_url: Option<String>,
    pub(crate) episode_link_bounds: Bounds,
    pub(crate) episode_201_link_bounds: Bounds,
    pub(crate) landing_live: Option<LandingLive>,
    pub(crate) user: UserInfo,
    pub(crate) loading: bool,
    pub(crate) view: AppView,
    pub(crate) gpu_context: Option<GpuContext>,
    pub(crate) repos: Vec<RepoInfo>,
    pub(crate) repos_loading: bool,
    pub(crate) hovered_repo_idx: Option<usize>,
    pub(crate) repo_bounds: Vec<Bounds>,
    pub(crate) selected_repo: Option<String>,
    pub(crate) scroll_offset: f32,
    pub(crate) hud_context: Option<HudContext>,
    pub(crate) hud_ui: HudUi,
    pub(crate) hud_layout: HudLayout,
    pub(crate) hud_stream: Option<HudStreamHandle>,
    pub(crate) hud_settings_loaded: bool,
    pub(crate) hud_metrics_polling: bool,
    pub(crate) hud_metrics_timer: Option<i32>,
    pub(crate) open_share_after_start: bool,
    pub(crate) funnel_landing_tracked: bool,
    pub(crate) wallet: WalletUi,
    // Bazaar market feed state
    pub(crate) market_jobs: Vec<MarketJob>,
    pub(crate) market_stats: MarketStats,
    pub(crate) left_cta_bounds: Bounds,
    pub(crate) right_cta_bounds: Bounds,
    pub(crate) left_cta_hovered: bool,
    pub(crate) right_cta_hovered: bool,
    pub(crate) hovered_job_idx: Option<usize>,
    pub(crate) job_bounds: Vec<Bounds>,
    // NIP-90 events pane
    pub(crate) nip90: Nip90State,
    pub(crate) nip90_relay_handle: Option<NostrRelayHandle>,
    pub(crate) nip90_event_bounds: Vec<Bounds>,
    // DVM directory (NIP-89)
    pub(crate) dvm_directory: DvmDirectoryState,
    pub(crate) dvm_tab_bounds: [Bounds; 2], // [Feed, DVMs] tab bounds
    pub(crate) dvm_content_bounds: Bounds,  // Scrollable content area for DVM marketplace
    // Global notes feed (NIP-01)
    pub(crate) global_feed: GlobalFeedState,
    pub(crate) global_feed_bounds: Bounds,           // Scrollable content area
    pub(crate) global_feed_note_bounds: Vec<Bounds>, // Per-note bounds for click detection
    // Bazaar real jobs (NIP-90 kinds 5930-5933)
    pub(crate) bazaar: BazaarState,
    pub(crate) bazaar_job_bounds: Vec<Bounds>,
    pub(crate) bazaar_scroll_bounds: Bounds,
    // CTA card frame animators
    pub(crate) left_cta_animator: FrameAnimator,
    pub(crate) right_cta_animator: FrameAnimator,
    pub(crate) cta_frames_started: bool,
    // Background dots grid
    pub(crate) dots_grid: DotsGrid,
    pub(crate) file_entries: Vec<FileEntry>,
    pub(crate) file_entry_bounds: Vec<Bounds>,
    pub(crate) file_list_bounds: Bounds,
    pub(crate) file_open_bounds: Bounds,
    pub(crate) file_save_bounds: Bounds,
    pub(crate) file_open_hovered: bool,
    pub(crate) file_save_hovered: bool,
    pub(crate) hovered_file_idx: Option<usize>,
    pub(crate) file_scroll_offset: f32,
    pub(crate) file_status: Option<String>,
    pub(crate) markdown_demo: MarkdownDemo,
    pub(crate) editor_workspace: EditorWorkspace,
    // Autopilot chat overlay
    pub(crate) autopilot_chat: crate::autopilot_chat::AutopilotChatPane,
    // Claude chat overlay + tunnel state
    pub(crate) claude_chat: crate::claude_chat::ClaudeChatPane,
    pub(crate) claude_state: ClaudeAgentState,
    pub(crate) intro_agent_state: crate::intro_agent::IntroAgentState,
    // GFN (Group Forming Networks) page state
    pub(crate) gfn: GfnState,
    // 2026 page state
    pub(crate) y2026: Y2026State,
    // ML inference visualization page state
    pub(crate) ml_viz: MlVizState,
    // GPT-OSS visualization page state
    pub(crate) gptoss: GptOssVizState,
    // FM Bridge visualization page state
    pub(crate) fm_viz: FmVizState,
    // FRLM (Fracking Apple Silicon) power comparison page state
    pub(crate) frlm: FrlmState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mouse_pos: Point::ZERO,
            button_hovered: false,
            button_bounds: Bounds::ZERO,
            landing_issue_bounds: Bounds::ZERO,
            landing_issue_url: None,
            episode_link_bounds: Bounds::ZERO,
            episode_201_link_bounds: Bounds::ZERO,
            landing_live: None,
            user: UserInfo::default(),
            loading: true,
            view: AppView::Landing,
            gpu_context: None,
            repos: Vec::new(),
            repos_loading: false,
            hovered_repo_idx: None,
            repo_bounds: Vec::new(),
            selected_repo: None,
            scroll_offset: 0.0,
            hud_context: None,
            hud_ui: HudUi::new(),
            hud_layout: HudLayout::default(),
            hud_stream: None,
            hud_settings_loaded: false,
            hud_metrics_polling: false,
            hud_metrics_timer: None,
            open_share_after_start: false,
            funnel_landing_tracked: false,
            wallet: WalletUi::new(),
            market_jobs: dummy_market_jobs(),
            market_stats: MarketStats::default(),
            left_cta_bounds: Bounds::ZERO,
            right_cta_bounds: Bounds::ZERO,
            left_cta_hovered: false,
            right_cta_hovered: false,
            hovered_job_idx: None,
            job_bounds: Vec::new(),
            nip90: Nip90State::new(),
            nip90_relay_handle: None,
            nip90_event_bounds: Vec::new(),
            dvm_directory: DvmDirectoryState::new(),
            dvm_tab_bounds: [Bounds::ZERO; 2],
            dvm_content_bounds: Bounds::ZERO,
            global_feed: GlobalFeedState::new(),
            global_feed_bounds: Bounds::ZERO,
            global_feed_note_bounds: Vec::new(),
            bazaar: BazaarState::new(),
            bazaar_job_bounds: Vec::new(),
            bazaar_scroll_bounds: Bounds::ZERO,
            left_cta_animator: FrameAnimator::new(),
            right_cta_animator: FrameAnimator::new(),
            cta_frames_started: false,
            dots_grid: DotsGrid::new(),
            file_entries: Vec::new(),
            file_entry_bounds: Vec::new(),
            file_list_bounds: Bounds::ZERO,
            file_open_bounds: Bounds::ZERO,
            file_save_bounds: Bounds::ZERO,
            file_open_hovered: false,
            file_save_hovered: false,
            hovered_file_idx: None,
            file_scroll_offset: 0.0,
            file_status: None,
            markdown_demo: MarkdownDemo::new(),
            editor_workspace: EditorWorkspace::new(),
            autopilot_chat: crate::autopilot_chat::AutopilotChatPane::new(),
            claude_chat: crate::claude_chat::ClaudeChatPane::new(),
            claude_state: ClaudeAgentState::default(),
            intro_agent_state: crate::intro_agent::IntroAgentState::default(),
            gfn: GfnState::default(),
            y2026: Y2026State::default(),
            ml_viz: MlVizState::default(),
            gptoss: GptOssVizState::default(),
            fm_viz: FmVizState::default(),
            frlm: FrlmState::default(),
        }
    }
}

fn demo_markdown_source() -> String {
    let readme = include_str!("../../docs/README.md");
    let (lang, code) = extract_code_block(readme, 12)
        .unwrap_or_else(|| ("text".to_string(), String::new()));

    let mut markdown = String::new();
    markdown.push_str("## Quick Start\n");
    markdown.push_str("From `crates/web/docs/README.md`\n\n");
    markdown.push_str("```");
    markdown.push_str(&lang);
    markdown.push('\n');
    if !code.is_empty() {
        markdown.push_str(&code);
        if !code.ends_with('\n') {
            markdown.push('\n');
        }
    }
    markdown.push_str("```\n");
    markdown
}

fn extract_code_block(source: &str, max_lines: usize) -> Option<(String, String)> {
    for lang in ["bash", "sh", "shell"] {
        if let Some(code) = extract_fenced_block(source, lang, max_lines) {
            return Some((lang.to_string(), code));
        }
    }

    extract_first_block(source, max_lines)
}

fn extract_fenced_block(source: &str, lang: &str, max_lines: usize) -> Option<String> {
    let fence = format!("```{}", lang);
    let mut in_block = false;
    let mut lines = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim_start();
        if !in_block {
            if trimmed.starts_with(&fence) {
                in_block = true;
            }
            continue;
        }

        if trimmed.starts_with("```") {
            break;
        }

        lines.push(line);
        if lines.len() >= max_lines {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n").trim_matches('\n').to_string())
    }
}

fn extract_first_block(source: &str, max_lines: usize) -> Option<(String, String)> {
    let mut in_block = false;
    let mut lang = String::new();
    let mut lines = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim_start();
        if !in_block {
            if let Some(rest) = trimmed.strip_prefix("```") {
                lang = rest.trim().to_string();
                in_block = true;
            }
            continue;
        }

        if trimmed.starts_with("```") {
            break;
        }

        lines.push(line);
        if lines.len() >= max_lines {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        let language = if lang.is_empty() { "text" } else { lang.as_str() };
        Some((language.to_string(), lines.join("\n").trim_matches('\n').to_string()))
    }
}

fn tokenize_markdown(source: &str) -> VecDeque<String> {
    source
        .chars()
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

fn demo_editor_source() -> String {
    let source = include_str!("views/landing.rs");
    source
        .lines()
        .take(160)
        .collect::<Vec<_>>()
        .join("\n")
}
