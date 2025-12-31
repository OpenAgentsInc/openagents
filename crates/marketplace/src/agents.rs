//! Types for agents as autonomous economic actors in the marketplace

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent identity and profile
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    /// Unique agent ID (Nostr public key)
    pub id: String,
    /// Human-readable agent name
    pub name: String,
    /// Agent's wallet for economic operations
    pub wallet: AgentWallet,
    /// Skills this agent can perform
    pub skills: Vec<String>,
    /// MCP servers this agent has access to
    pub mcp_servers: Vec<String>,
    /// Agent's area of specialization
    pub specialization: Option<String>,
    /// Coalitions this agent is actively participating in
    pub active_coalitions: Vec<String>,
    /// Agent's reputation score in coalitions (0.0-1.0)
    pub coalition_reputation: f32,
    /// When this agent was created
    pub created_at: DateTime<Utc>,
    /// Who sponsored this agent's creation (human or parent agent)
    pub sponsor: Option<String>,
}

/// Agent wallet for economic transactions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWallet {
    /// Current balance in satoshis
    pub balance_sats: u64,
    /// Lightning address for receiving payments
    pub lightning_address: String,
    /// Daily operating cost in satoshis
    pub daily_burn_sats: u64,
    /// Total lifetime earnings in satoshis
    pub lifetime_earnings_sats: u64,
}

impl AgentWallet {
    /// Create a new wallet with initial balance
    pub fn new(lightning_address: impl Into<String>, initial_balance_sats: u64) -> Self {
        Self {
            balance_sats: initial_balance_sats,
            lightning_address: lightning_address.into(),
            daily_burn_sats: 0,
            lifetime_earnings_sats: 0,
        }
    }

    /// Check if agent can afford to operate for N days
    pub fn can_operate_for_days(&self, days: u32) -> bool {
        let required = self.daily_burn_sats * (days as u64);
        self.balance_sats >= required
    }

    /// Calculate days until agent runs out of funds
    pub fn days_until_broke(&self) -> Option<u32> {
        if self.daily_burn_sats == 0 {
            return None; // Infinite if no burn rate
        }
        Some((self.balance_sats / self.daily_burn_sats) as u32)
    }
}

/// Agent lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent is active and operational
    Active,
    /// Agent is low on funds and may die soon
    LowBalance,
    /// Agent has run out of funds and is terminated
    Terminated,
    /// Agent has been suspended due to policy violation
    Suspended,
}

impl AgentStatus {
    /// Check if agent can perform work
    pub fn is_operational(&self) -> bool {
        matches!(self, AgentStatus::Active | AgentStatus::LowBalance)
    }

    /// Get status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentStatus::Active => "active",
            AgentStatus::LowBalance => "low_balance",
            AgentStatus::Terminated => "terminated",
            AgentStatus::Suspended => "suspended",
        }
    }
}

/// Request to spawn a new agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpawnRequest {
    /// Agent name
    pub name: String,
    /// Initial funding in satoshis
    pub bootstrap_sats: u64,
    /// Capabilities/skills this agent should have
    pub capabilities: Vec<String>,
    /// Who is sponsoring this agent
    pub sponsor: String,
}

/// Pricing model for agent services
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PricingModel {
    /// Fixed price per task
    PerTask {
        /// Base price in satoshis
        base_sats: u64,
    },
    /// Hourly rate
    Hourly {
        /// Satoshis per hour
        sats_per_hour: u64,
    },
    /// Per-unit pricing (e.g., per API call, per token)
    PerUnit {
        /// Satoshis per unit
        sats_per_unit: u64,
    },
    /// Custom negotiated pricing
    Negotiated,
}

/// Agent pricing structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPricing {
    /// Pricing model used
    pub model: PricingModel,
    /// Base price in satoshis
    pub base_sats: u64,
    /// Optional per-unit rate in satoshis
    pub per_unit_sats: Option<u64>,
    /// Maximum price cap in satoshis
    pub max_sats: Option<u64>,
}

impl AgentPricing {
    /// Create a new per-task pricing
    pub fn per_task(base_sats: u64) -> Self {
        Self {
            model: PricingModel::PerTask { base_sats },
            base_sats,
            per_unit_sats: None,
            max_sats: None,
        }
    }

    /// Create a new hourly pricing
    pub fn hourly(sats_per_hour: u64) -> Self {
        Self {
            model: PricingModel::Hourly { sats_per_hour },
            base_sats: sats_per_hour,
            per_unit_sats: None,
            max_sats: None,
        }
    }

    /// Create a new per-unit pricing
    pub fn per_unit(sats_per_unit: u64) -> Self {
        Self {
            model: PricingModel::PerUnit { sats_per_unit },
            base_sats: 0,
            per_unit_sats: Some(sats_per_unit),
            max_sats: None,
        }
    }

    /// Set maximum price cap
    pub fn with_max(mut self, max_sats: u64) -> Self {
        self.max_sats = Some(max_sats);
        self
    }
}

/// Agent availability status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentAvailability {
    /// Available for hire
    Available,
    /// Busy with current task
    Busy,
    /// Offline or unavailable
    Offline,
}

impl AgentAvailability {
    /// Check if agent can take new work
    pub fn is_available(&self) -> bool {
        matches!(self, AgentAvailability::Available)
    }

    /// Get availability as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentAvailability::Available => "available",
            AgentAvailability::Busy => "busy",
            AgentAvailability::Offline => "offline",
        }
    }
}

/// Agent listing for marketplace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentListing {
    /// Agent ID
    pub agent_id: String,
    /// Service name
    pub service_name: String,
    /// Service description
    pub description: String,
    /// Agent specializations
    pub specializations: Vec<String>,
    /// Pricing information
    pub pricing: AgentPricing,
    /// Current availability
    pub availability: AgentAvailability,
}

impl AgentListing {
    /// Create a new agent listing
    pub fn new(
        agent_id: impl Into<String>,
        service_name: impl Into<String>,
        description: impl Into<String>,
        pricing: AgentPricing,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            service_name: service_name.into(),
            description: description.into(),
            specializations: Vec::new(),
            pricing,
            availability: AgentAvailability::Available,
        }
    }

    /// Add a specialization
    pub fn with_specialization(mut self, spec: impl Into<String>) -> Self {
        self.specializations.push(spec.into());
        self
    }

    /// Set availability
    pub fn with_availability(mut self, availability: AgentAvailability) -> Self {
        self.availability = availability;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_wallet_new() {
        let wallet = AgentWallet::new("agent@ln.address", 100_000);
        assert_eq!(wallet.balance_sats, 100_000);
        assert_eq!(wallet.lightning_address, "agent@ln.address");
        assert_eq!(wallet.daily_burn_sats, 0);
        assert_eq!(wallet.lifetime_earnings_sats, 0);
    }

    #[test]
    fn test_agent_wallet_can_operate() {
        let mut wallet = AgentWallet::new("agent@ln.address", 10_000);
        wallet.daily_burn_sats = 1_000;

        assert!(wallet.can_operate_for_days(10));
        assert!(wallet.can_operate_for_days(5));
        assert!(!wallet.can_operate_for_days(11));
        assert!(!wallet.can_operate_for_days(100));
    }

    #[test]
    fn test_agent_wallet_days_until_broke() {
        let mut wallet = AgentWallet::new("agent@ln.address", 10_000);
        wallet.daily_burn_sats = 1_000;

        assert_eq!(wallet.days_until_broke(), Some(10));

        wallet.daily_burn_sats = 0;
        assert_eq!(wallet.days_until_broke(), None);

        wallet.daily_burn_sats = 500;
        assert_eq!(wallet.days_until_broke(), Some(20));
    }

    #[test]
    fn test_agent_status_is_operational() {
        assert!(AgentStatus::Active.is_operational());
        assert!(AgentStatus::LowBalance.is_operational());
        assert!(!AgentStatus::Terminated.is_operational());
        assert!(!AgentStatus::Suspended.is_operational());
    }

    #[test]
    fn test_agent_status_as_str() {
        assert_eq!(AgentStatus::Active.as_str(), "active");
        assert_eq!(AgentStatus::LowBalance.as_str(), "low_balance");
        assert_eq!(AgentStatus::Terminated.as_str(), "terminated");
        assert_eq!(AgentStatus::Suspended.as_str(), "suspended");
    }

    #[test]
    fn test_agent_availability_is_available() {
        assert!(AgentAvailability::Available.is_available());
        assert!(!AgentAvailability::Busy.is_available());
        assert!(!AgentAvailability::Offline.is_available());
    }

    #[test]
    fn test_agent_availability_as_str() {
        assert_eq!(AgentAvailability::Available.as_str(), "available");
        assert_eq!(AgentAvailability::Busy.as_str(), "busy");
        assert_eq!(AgentAvailability::Offline.as_str(), "offline");
    }

    #[test]
    fn test_agent_pricing_builders() {
        let per_task = AgentPricing::per_task(1_000);
        assert_eq!(per_task.base_sats, 1_000);
        assert!(matches!(per_task.model, PricingModel::PerTask { .. }));

        let hourly = AgentPricing::hourly(5_000);
        assert_eq!(hourly.base_sats, 5_000);
        assert!(matches!(hourly.model, PricingModel::Hourly { .. }));

        let per_unit = AgentPricing::per_unit(100);
        assert_eq!(per_unit.base_sats, 0);
        assert_eq!(per_unit.per_unit_sats, Some(100));
        assert!(matches!(per_unit.model, PricingModel::PerUnit { .. }));
    }

    #[test]
    fn test_agent_pricing_with_max() {
        let pricing = AgentPricing::per_task(1_000).with_max(10_000);
        assert_eq!(pricing.max_sats, Some(10_000));
    }

    #[test]
    fn test_agent_listing_builder() {
        let listing = AgentListing::new(
            "agent123",
            "Code Review Service",
            "Expert code reviewer",
            AgentPricing::per_task(5_000),
        )
        .with_specialization("rust")
        .with_specialization("typescript")
        .with_availability(AgentAvailability::Available);

        assert_eq!(listing.agent_id, "agent123");
        assert_eq!(listing.service_name, "Code Review Service");
        assert_eq!(listing.specializations.len(), 2);
        assert_eq!(listing.availability, AgentAvailability::Available);
    }

    #[test]
    fn test_agent_spawn_request_serde() {
        let request = AgentSpawnRequest {
            name: "test-agent".to_string(),
            bootstrap_sats: 100_000,
            capabilities: vec!["coding".to_string(), "testing".to_string()],
            sponsor: "human123".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: AgentSpawnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, request);
    }

    #[test]
    fn test_agent_status_serde() {
        let status = AgentStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"active\"");
        let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    #[test]
    fn test_pricing_model_serde() {
        let model = PricingModel::PerTask { base_sats: 1_000 };
        let json = serde_json::to_string(&model).unwrap();
        let deserialized: PricingModel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, model);
    }

    #[test]
    fn test_agent_pricing_serde() {
        let pricing = AgentPricing::hourly(5_000).with_max(50_000);
        let json = serde_json::to_string(&pricing).unwrap();
        let deserialized: AgentPricing = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pricing);
    }

    #[test]
    fn test_agent_listing_serde() {
        let listing = AgentListing::new(
            "agent123",
            "Code Review",
            "Reviews code",
            AgentPricing::per_task(5_000),
        );
        let json = serde_json::to_string(&listing).unwrap();
        let deserialized: AgentListing = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_id, listing.agent_id);
    }
}
