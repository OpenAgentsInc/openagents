//! Agent configuration types
//!
//! These types define agent configuration that is persisted to disk
//! and used to manage agent lifecycle.

use serde::{Deserialize, Serialize};

/// Lifecycle state of a sovereign agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// Agent is being initialized, waiting for funding
    Spawning,
    /// Agent is fully operational
    Active,
    /// Balance is below 7-day runway threshold
    LowBalance,
    /// Agent has paused operations to conserve funds
    Hibernating,
    /// Agent has run out of funds and is terminated
    Dead,
}

impl Default for LifecycleState {
    fn default() -> Self {
        LifecycleState::Spawning
    }
}

/// Autonomy level for agent operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Human approves every action
    Supervised,
    /// Acts within policy limits
    Bounded,
    /// Independent within policy
    Autonomous,
}

impl Default for AutonomyLevel {
    fn default() -> Self {
        AutonomyLevel::Bounded
    }
}

impl std::str::FromStr for AutonomyLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "supervised" => Ok(AutonomyLevel::Supervised),
            "bounded" => Ok(AutonomyLevel::Bounded),
            "autonomous" => Ok(AutonomyLevel::Autonomous),
            _ => Err(format!("invalid autonomy level: {}", s)),
        }
    }
}

/// Agent profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    /// Agent display name
    pub name: String,
    /// Agent description
    pub about: String,
    /// Autonomy level
    pub autonomy: AutonomyLevel,
    /// Capabilities (e.g., "research", "coding", "trading")
    pub capabilities: Vec<String>,
    /// Protocol version
    pub version: String,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            name: "SovereignAgent".to_string(),
            about: "A sovereign AI agent".to_string(),
            autonomy: AutonomyLevel::default(),
            capabilities: vec!["general".to_string()],
            version: "1.0.0".to_string(),
        }
    }
}

/// Agent schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    /// Heartbeat interval in seconds (default: 900 = 15 minutes)
    pub heartbeat_seconds: u64,
    /// Event triggers (e.g., "mention", "dm", "zap")
    pub triggers: Vec<String>,
    /// Whether schedule is active
    pub active: bool,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            heartbeat_seconds: 900, // 15 minutes
            triggers: vec!["mention".to_string(), "dm".to_string(), "zap".to_string()],
            active: true,
        }
    }
}

/// Runway and budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunwayConfig {
    /// Minimum days of runway before entering LowBalance state
    pub low_balance_days: u32,
    /// Estimated daily burn rate in satoshis
    pub daily_burn_sats: u64,
    /// Minimum balance to hibernate (not die)
    pub hibernate_threshold_sats: u64,
    /// Daily spending limit in satoshis
    pub daily_limit_sats: u64,
    /// Per-tick spending limit in satoshis
    pub per_tick_limit_sats: u64,
}

impl Default for RunwayConfig {
    fn default() -> Self {
        Self {
            low_balance_days: 7,
            daily_burn_sats: 1000, // ~1000 sats/day default burn rate
            hibernate_threshold_sats: 1000,
            daily_limit_sats: 10_000,
            per_tick_limit_sats: 1000,
        }
    }
}

/// Network configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkConfig {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig::Regtest
    }
}

impl std::str::FromStr for NetworkConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mainnet" => Ok(NetworkConfig::Mainnet),
            "testnet" => Ok(NetworkConfig::Testnet),
            "signet" => Ok(NetworkConfig::Signet),
            "regtest" => Ok(NetworkConfig::Regtest),
            _ => Err(format!("invalid network: {}", s)),
        }
    }
}

/// Complete agent configuration stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent display name
    pub name: String,
    /// Nostr public key (hex)
    pub pubkey: String,
    /// npub (bech32)
    pub npub: String,
    /// Mnemonic (encrypted at rest)
    pub mnemonic_encrypted: String,
    /// Spark address for receiving funds
    pub spark_address: String,
    /// Network (mainnet, testnet, regtest)
    pub network: NetworkConfig,
    /// Relay URLs
    pub relays: Vec<String>,
    /// Creation timestamp (unix)
    pub created_at: u64,
    /// Current lifecycle state
    pub state: LifecycleState,
    /// Last active timestamp (unix)
    pub last_active_at: u64,
    /// Total ticks executed
    pub tick_count: u64,
    /// Profile configuration
    pub profile: ProfileConfig,
    /// Schedule configuration
    pub schedule: ScheduleConfig,
    /// Runway configuration
    pub runway: RunwayConfig,
}

impl AgentConfig {
    /// Create a new agent config
    pub fn new(
        name: String,
        pubkey: String,
        npub: String,
        mnemonic_encrypted: String,
        spark_address: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp() as u64;
        Self {
            name: name.clone(),
            pubkey,
            npub,
            mnemonic_encrypted,
            spark_address,
            network: NetworkConfig::default(),
            relays: vec!["wss://relay.damus.io".to_string()],
            created_at: now,
            state: LifecycleState::Spawning,
            last_active_at: now,
            tick_count: 0,
            profile: ProfileConfig {
                name,
                ..Default::default()
            },
            schedule: ScheduleConfig::default(),
            runway: RunwayConfig::default(),
        }
    }

    /// Check if agent is operational (can run ticks)
    pub fn is_operational(&self) -> bool {
        matches!(
            self.state,
            LifecycleState::Active | LifecycleState::LowBalance
        )
    }

    /// Check if agent is dead
    pub fn is_dead(&self) -> bool {
        matches!(self.state, LifecycleState::Dead)
    }

    /// Check if agent needs funding
    pub fn needs_funding(&self) -> bool {
        matches!(
            self.state,
            LifecycleState::Spawning | LifecycleState::LowBalance | LifecycleState::Hibernating
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_state_serialization() {
        assert_eq!(
            serde_json::to_string(&LifecycleState::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&LifecycleState::LowBalance).unwrap(),
            "\"low_balance\""
        );
    }

    #[test]
    fn test_autonomy_level_from_str() {
        assert_eq!(
            "bounded".parse::<AutonomyLevel>().unwrap(),
            AutonomyLevel::Bounded
        );
        assert_eq!(
            "AUTONOMOUS".parse::<AutonomyLevel>().unwrap(),
            AutonomyLevel::Autonomous
        );
        assert!("invalid".parse::<AutonomyLevel>().is_err());
    }

    #[test]
    fn test_agent_config_operational() {
        let mut config = AgentConfig::new(
            "Test".to_string(),
            "abc".to_string(),
            "npub1abc".to_string(),
            "encrypted".to_string(),
            "sp1abc".to_string(),
        );

        assert!(!config.is_operational()); // Spawning
        config.state = LifecycleState::Active;
        assert!(config.is_operational());
        config.state = LifecycleState::LowBalance;
        assert!(config.is_operational());
        config.state = LifecycleState::Dead;
        assert!(!config.is_operational());
    }
}
