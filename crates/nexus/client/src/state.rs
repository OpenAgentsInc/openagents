use serde::Deserialize;
use wgpui::components::hud::FrameAnimator;
use wgpui::Bounds;

/// Connection status for stats fetching
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// A kind with its count
#[derive(Clone, Debug, Deserialize)]
pub struct KindCount {
    pub kind: u16,
    pub count: u64,
}

/// Event stats from API
#[derive(Clone, Debug, Default, Deserialize)]
pub struct EventStats {
    pub total: u64,
    pub last_24h: u64,
    #[serde(default)]
    pub by_kind: Vec<KindCount>,
}

/// Job stats from API
#[derive(Clone, Debug, Default, Deserialize)]
pub struct JobStats {
    pub pending: u64,
    pub completed_24h: u64,
    #[serde(default)]
    pub by_kind: Vec<KindCount>,
}

/// RLM (Recursive Language Model) stats from API
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RlmStats {
    #[serde(default)]
    pub subqueries_total: u64,
    #[serde(default)]
    pub subqueries_24h: u64,
    #[serde(default)]
    pub results_total: u64,
    #[serde(default)]
    pub results_24h: u64,
    #[serde(default)]
    pub providers_active: u64,
}

/// Full stats response from /api/stats
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RelayStats {
    #[serde(default)]
    pub events: EventStats,
    #[serde(default)]
    pub jobs: JobStats,
    #[serde(default)]
    pub rlm: RlmStats,
    #[serde(default)]
    pub timestamp: u64,
}

/// State for the Nexus HUD
pub struct NexusState {
    // Frame animation
    pub frame_animator: FrameAnimator,
    pub frame_started: bool,

    // Connection status
    pub connection_status: ConnectionStatus,
    pub last_fetch_time: u64,
    pub fetch_error: Option<String>,
    pub fetch_count: u32,

    // Stats data
    pub stats: RelayStats,

    // UI state
    pub scroll_offset: f32,
    pub content_bounds: Bounds,
    pub refresh_bounds: Bounds,
    pub refresh_hovered: bool,
}

impl Default for NexusState {
    fn default() -> Self {
        Self {
            frame_animator: FrameAnimator::new(),
            frame_started: false,
            connection_status: ConnectionStatus::Disconnected,
            last_fetch_time: 0,
            fetch_error: None,
            fetch_count: 0,
            stats: RelayStats::default(),
            scroll_offset: 0.0,
            content_bounds: Bounds::ZERO,
            refresh_bounds: Bounds::ZERO,
            refresh_hovered: false,
        }
    }
}
