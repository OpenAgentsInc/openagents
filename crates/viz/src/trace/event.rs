//! Unified trace event taxonomy

use serde::{Deserialize, Serialize};

/// Execution venue
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Venue {
    /// Browser-based WASM execution
    Browser,
    /// Local pylon server
    PylonLocal,
    /// Swarm provider network
    Swarm { provider_id: String },
    /// Apple Foundation Models (on-device)
    AppleFM,
    /// Remote GPU cluster
    RemoteGPU { cluster_id: String },
}

impl Venue {
    pub fn label(&self) -> &str {
        match self {
            Venue::Browser => "Browser",
            Venue::PylonLocal => "Pylon",
            Venue::Swarm { .. } => "Swarm",
            Venue::AppleFM => "Apple FM",
            Venue::RemoteGPU { .. } => "GPU",
        }
    }
}

/// Unified trace event for all inference backends
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TraceEvent {
    // Session lifecycle
    SessionStart {
        id: String,
        venue: Venue,
    },
    SessionEnd {
        id: String,
        duration_ms: u64,
    },

    // Inference requests
    RequestStart {
        id: String,
        venue: Venue,
        model: String,
    },
    RequestEnd {
        id: String,
        tokens_in: u32,
        tokens_out: u32,
        cost_usd: f64,
    },

    // Token streaming
    TokenGenerated {
        token: String,
        prob: f32,
        entropy: f32,
    },

    // Tool calls
    ToolCallStart {
        id: String,
        tool: String,
    },
    ToolCallEnd {
        id: String,
        success: bool,
        duration_ms: u64,
    },

    // Routing decisions
    RoutingDecision {
        venue: Venue,
        reason: String,
        alternatives: Vec<Venue>,
    },

    // Resource events
    MemoryAlloc {
        bytes: u64,
        label: String,
    },
    MemoryFree {
        bytes: u64,
        label: String,
    },
    CacheHit {
        key: String,
    },
    CacheMiss {
        key: String,
    },

    // Loading/streaming
    FetchStart {
        url: String,
        total_bytes: u64,
    },
    FetchProgress {
        url: String,
        loaded: u64,
    },
    FetchComplete {
        url: String,
    },

    // GPU events
    KernelDispatch {
        name: String,
    },
    KernelComplete {
        name: String,
        duration_us: u64,
    },

    // Model loading
    ModelLoadStart {
        model: String,
        venue: Venue,
    },
    ModelLoadProgress {
        model: String,
        layer: u32,
        total_layers: u32,
    },
    ModelLoadComplete {
        model: String,
        duration_ms: u64,
    },

    // Errors
    Error {
        code: String,
        message: String,
    },
}

impl TraceEvent {
    /// Get a short label for this event type
    pub fn label(&self) -> &'static str {
        match self {
            TraceEvent::SessionStart { .. } => "session.start",
            TraceEvent::SessionEnd { .. } => "session.end",
            TraceEvent::RequestStart { .. } => "request.start",
            TraceEvent::RequestEnd { .. } => "request.end",
            TraceEvent::TokenGenerated { .. } => "token",
            TraceEvent::ToolCallStart { .. } => "tool.start",
            TraceEvent::ToolCallEnd { .. } => "tool.end",
            TraceEvent::RoutingDecision { .. } => "routing",
            TraceEvent::MemoryAlloc { .. } => "mem.alloc",
            TraceEvent::MemoryFree { .. } => "mem.free",
            TraceEvent::CacheHit { .. } => "cache.hit",
            TraceEvent::CacheMiss { .. } => "cache.miss",
            TraceEvent::FetchStart { .. } => "fetch.start",
            TraceEvent::FetchProgress { .. } => "fetch.progress",
            TraceEvent::FetchComplete { .. } => "fetch.complete",
            TraceEvent::KernelDispatch { .. } => "kernel.dispatch",
            TraceEvent::KernelComplete { .. } => "kernel.complete",
            TraceEvent::ModelLoadStart { .. } => "model.load.start",
            TraceEvent::ModelLoadProgress { .. } => "model.load.progress",
            TraceEvent::ModelLoadComplete { .. } => "model.load.complete",
            TraceEvent::Error { .. } => "error",
        }
    }

    /// Is this an error event?
    pub fn is_error(&self) -> bool {
        matches!(self, TraceEvent::Error { .. })
    }
}
