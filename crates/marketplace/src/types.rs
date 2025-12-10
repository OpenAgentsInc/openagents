//! Core types for the Marketplace screen

use gpui::Hsla;
use theme::trust;

/// The three sub-marketplaces
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarketplaceTab {
    #[default]
    Agents,
    Compute,
    Services,
}

impl MarketplaceTab {
    /// Display label for the tab
    pub fn label(&self) -> &'static str {
        match self {
            Self::Agents => "AGENTS",
            Self::Compute => "COMPUTE",
            Self::Services => "SERVICES",
        }
    }

    /// Icon/emoji for the tab
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Agents => "🤖",
            Self::Compute => "⚡",
            Self::Services => "🔧",
        }
    }

    /// All tabs for iteration
    pub fn all() -> &'static [MarketplaceTab] {
        &[Self::Agents, Self::Compute, Self::Services]
    }
}

/// Trust tier levels for marketplace progression
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrustTier {
    #[default]
    Bronze,
    Silver,
    Gold,
    Diamond,
}

impl TrustTier {
    /// Primary color for the tier
    pub fn color(&self) -> Hsla {
        match self {
            Self::Bronze => trust::BRONZE,
            Self::Silver => trust::SILVER,
            Self::Gold => trust::GOLD,
            Self::Diamond => trust::DIAMOND,
        }
    }

    /// Background color for the tier badge
    pub fn bg_color(&self) -> Hsla {
        match self {
            Self::Bronze => trust::BRONZE_BG,
            Self::Silver => trust::SILVER_BG,
            Self::Gold => trust::GOLD_BG,
            Self::Diamond => trust::DIAMOND_BG,
        }
    }

    /// Border color for the tier badge
    pub fn border_color(&self) -> Hsla {
        match self {
            Self::Bronze => trust::BRONZE_BORDER,
            Self::Silver => trust::SILVER_BORDER,
            Self::Gold => trust::GOLD_BORDER,
            Self::Diamond => trust::DIAMOND_BORDER,
        }
    }

    /// Display label for the tier
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bronze => "BRONZE",
            Self::Silver => "SILVER",
            Self::Gold => "GOLD",
            Self::Diamond => "DIAMOND",
        }
    }

    /// Score threshold for reaching this tier
    pub fn threshold(&self) -> u64 {
        match self {
            Self::Bronze => 0,
            Self::Silver => 500,
            Self::Gold => 2000,
            Self::Diamond => 10000,
        }
    }
}

// ============================================================================
// Agent Store Types
// ============================================================================

/// Agent category for filtering
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentCategory {
    #[default]
    All,
    Coding,
    Research,
    Creative,
    Data,
    Automation,
    Communication,
    Finance,
}

impl AgentCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Coding => "CODING",
            Self::Research => "RESEARCH",
            Self::Creative => "CREATIVE",
            Self::Data => "DATA",
            Self::Automation => "AUTOMATION",
            Self::Communication => "COMMS",
            Self::Finance => "FINANCE",
        }
    }

    pub fn all() -> &'static [AgentCategory] {
        &[
            Self::All,
            Self::Coding,
            Self::Research,
            Self::Creative,
            Self::Data,
            Self::Automation,
            Self::Communication,
            Self::Finance,
        ]
    }
}

/// Sort options for agent listings
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentSortOption {
    #[default]
    Trending,
    MostInstalls,
    HighestEarnings,
    BestRating,
    MostRecent,
    BenchmarkScore,
}

impl AgentSortOption {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Trending => "TRENDING",
            Self::MostInstalls => "MOST INSTALLS",
            Self::HighestEarnings => "TOP EARNINGS",
            Self::BestRating => "BEST RATED",
            Self::MostRecent => "NEWEST",
            Self::BenchmarkScore => "BENCHMARK",
        }
    }
}

/// An agent listing in the marketplace
#[derive(Clone, Debug)]
pub struct AgentListing {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author_pubkey: String,
    pub author_name: String,
    pub version: String,
    pub category: AgentCategory,
    pub trust_tier: TrustTier,
    pub terminal_bench_score: Option<f32>,
    pub gym_score: Option<f32>,
    pub installs: u64,
    pub earnings_total_sats: u64,
    pub revenue_share_percent: f32,
    pub rating: f32,
    pub review_count: u32,
}

impl AgentListing {
    /// Create a mock agent for UI development
    pub fn mock(id: &str, name: &str, category: AgentCategory, tier: TrustTier) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("A powerful {} agent for task automation.", category.label().to_lowercase()),
            author_pubkey: "npub1...".to_string(),
            author_name: "@openagents".to_string(),
            version: "1.0.0".to_string(),
            category,
            trust_tier: tier,
            terminal_bench_score: Some(94.0),
            gym_score: Some(91.0),
            installs: 12_500,
            earnings_total_sats: 1_200_000,
            revenue_share_percent: 15.0,
            rating: 4.8,
            review_count: 2_347,
        }
    }
}

// ============================================================================
// Compute Market Types
// ============================================================================

/// Status of a model being served
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelStatus {
    #[default]
    Ready,
    Processing,
    Loading,
    Error,
}

impl ModelStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Processing => "PROCESSING",
            Self::Loading => "LOADING",
            Self::Error => "ERROR",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Ready => "●",
            Self::Processing => "◐",
            Self::Loading => "○",
            Self::Error => "✕",
        }
    }
}

/// A model currently available for compute jobs
#[derive(Clone, Debug)]
pub struct ActiveModel {
    pub name: String,
    pub provider: String,
    pub device: String,
    pub requests_per_hour: f32,
    pub earnings_per_hour_sats: u64,
    pub status: ModelStatus,
}

impl ActiveModel {
    pub fn mock(name: &str, device: &str, status: ModelStatus) -> Self {
        Self {
            name: name.to_string(),
            provider: "ollama".to_string(),
            device: device.to_string(),
            requests_per_hour: 45.0,
            earnings_per_hour_sats: 234,
            status,
        }
    }
}

/// Time range for earnings charts
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeRange {
    Hour,
    #[default]
    Day,
    Week,
    Month,
    All,
}

impl TimeRange {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Hour => "1H",
            Self::Day => "24H",
            Self::Week => "7D",
            Self::Month => "30D",
            Self::All => "ALL",
        }
    }

    pub fn all() -> &'static [TimeRange] {
        &[Self::Hour, Self::Day, Self::Week, Self::Month, Self::All]
    }
}

/// A data point for earnings charts
#[derive(Clone, Debug)]
pub struct EarningsDataPoint {
    pub label: String,
    pub sats: u64,
}

// ============================================================================
// Services Market Types
// ============================================================================

/// Service category for DVMs and MCPs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServiceCategory {
    #[default]
    All,
    Inference,
    Transcription,
    ImageGeneration,
    Translation,
    Analysis,
    Search,
    Storage,
}

impl ServiceCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Inference => "INFERENCE",
            Self::Transcription => "TRANSCRIPTION",
            Self::ImageGeneration => "IMAGE GEN",
            Self::Translation => "TRANSLATION",
            Self::Analysis => "ANALYSIS",
            Self::Search => "SEARCH",
            Self::Storage => "STORAGE",
        }
    }

    pub fn all() -> &'static [ServiceCategory] {
        &[
            Self::All,
            Self::Inference,
            Self::Transcription,
            Self::ImageGeneration,
            Self::Translation,
            Self::Analysis,
            Self::Search,
            Self::Storage,
        ]
    }
}

/// Pricing unit for services
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PricingUnit {
    #[default]
    PerRequest,
    PerMinute,
    Per1KTokens,
    PerImage,
    PerMB,
}

impl PricingUnit {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PerRequest => "/req",
            Self::PerMinute => "/min",
            Self::Per1KTokens => "/1K tok",
            Self::PerImage => "/img",
            Self::PerMB => "/MB",
        }
    }
}

/// A Data Vending Machine listing
#[derive(Clone, Debug)]
pub struct DVMListing {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: u32,
    pub provider_pubkey: String,
    pub provider_name: String,
    pub sats_per_unit: u64,
    pub pricing_unit: PricingUnit,
    pub rating: f32,
    pub request_count: u64,
    pub avg_latency_ms: u64,
}

impl DVMListing {
    pub fn mock(name: &str, kind: u32, sats: u64, unit: PricingUnit) -> Self {
        Self {
            id: format!("dvm_{}", name.to_lowercase().replace(' ', "_")),
            name: name.to_string(),
            description: format!("High-quality {} service.", name.to_lowercase()),
            kind,
            provider_pubkey: "npub1...".to_string(),
            provider_name: format!("@{}_dvm", name.to_lowercase().replace(' ', "_")),
            sats_per_unit: sats,
            pricing_unit: unit,
            rating: 4.7,
            request_count: 8_200,
            avg_latency_ms: 1_200,
        }
    }
}

/// An MCP server listing
#[derive(Clone, Debug)]
pub struct MCPServerListing {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tool_count: u32,
    pub provider_name: String,
    pub sats_per_unit: u64,
    pub pricing_unit: PricingUnit,
    pub installs: u64,
}

impl MCPServerListing {
    pub fn mock(name: &str, tools: u32, sats: u64) -> Self {
        Self {
            id: format!("mcp_{}", name.to_lowercase().replace(' ', "_")),
            name: name.to_string(),
            description: format!("{} tools for AI agents.", name),
            tool_count: tools,
            provider_name: "@mcp_provider".to_string(),
            sats_per_unit: sats,
            pricing_unit: PricingUnit::PerRequest,
            installs: 5_400,
        }
    }
}

// ============================================================================
// Activity Feed Types
// ============================================================================

/// Direction of a transaction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionDirection {
    Incoming,
    Outgoing,
}

/// A transaction in the activity feed
#[derive(Clone, Debug)]
pub struct Transaction {
    pub id: String,
    pub direction: TransactionDirection,
    pub amount_sats: u64,
    pub description: String,
    pub counterparty: String,
    pub timestamp: String,
}

impl Transaction {
    pub fn mock_incoming(desc: &str, amount: u64, time: &str) -> Self {
        Self {
            id: format!("tx_{}", desc.to_lowercase().replace(' ', "_")),
            direction: TransactionDirection::Incoming,
            amount_sats: amount,
            description: desc.to_string(),
            counterparty: "network".to_string(),
            timestamp: time.to_string(),
        }
    }

    pub fn mock_outgoing(desc: &str, amount: u64, time: &str) -> Self {
        Self {
            id: format!("tx_{}", desc.to_lowercase().replace(' ', "_")),
            direction: TransactionDirection::Outgoing,
            amount_sats: amount,
            description: desc.to_string(),
            counterparty: "service".to_string(),
            timestamp: time.to_string(),
        }
    }
}

/// Type of notification
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationKind {
    AgentInstalled,
    EarningsMilestone,
    TrustTierUp,
    SystemAlert,
    JobCompleted,
}

/// A notification in the activity feed
#[derive(Clone, Debug)]
pub struct Notification {
    pub id: String,
    pub kind: NotificationKind,
    pub title: String,
    pub message: String,
    pub read: bool,
    pub timestamp: String,
}
