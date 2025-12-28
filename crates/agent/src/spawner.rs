//! Agent Spawning
//!
//! Creates new sovereign agents with Nostr identity and Spark wallet.

use crate::config::{AgentConfig, AutonomyLevel, NetworkConfig, ProfileConfig, ScheduleConfig};
use crate::registry::{AgentRegistry, RegistryError};
use compute::domain::UnifiedIdentity;
use nostr::nip_sa::{
    AgentProfile, AgentProfileContent, AgentSchedule, AgentState, AgentStateContent,
    ThresholdConfig, KIND_AGENT_PROFILE, KIND_AGENT_SCHEDULE, KIND_AGENT_STATE,
};
use nostr::{Event, EventTemplate, finalize_event};
use nostr_client::RelayConnection;
use openagents_spark::{SparkWallet, WalletConfig};
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during agent spawning
#[derive(Debug, Error)]
pub enum SpawnError {
    #[error("identity error: {0}")]
    Identity(String),

    #[error("wallet error: {0}")]
    Wallet(String),

    #[error("relay error: {0}")]
    Relay(String),

    #[error("registry error: {0}")]
    Registry(#[from] RegistryError),

    #[error("event signing error: {0}")]
    Signing(String),

    #[error("agent already exists: {0}")]
    AlreadyExists(String),
}

/// Request to spawn a new agent
#[derive(Debug, Clone)]
pub struct SpawnRequest {
    /// Agent display name
    pub name: String,
    /// Agent description
    pub about: Option<String>,
    /// Capabilities (e.g., "research", "coding")
    pub capabilities: Vec<String>,
    /// Autonomy level
    pub autonomy: AutonomyLevel,
    /// Heartbeat interval in seconds
    pub heartbeat_seconds: u64,
    /// Event triggers
    pub triggers: Vec<String>,
    /// Network (mainnet, testnet, regtest)
    pub network: NetworkConfig,
    /// Relay URLs
    pub relays: Vec<String>,
}

impl Default for SpawnRequest {
    fn default() -> Self {
        Self {
            name: "SovereignAgent".to_string(),
            about: None,
            capabilities: vec!["general".to_string()],
            autonomy: AutonomyLevel::Bounded,
            heartbeat_seconds: 900, // 15 minutes
            triggers: vec!["mention".to_string(), "dm".to_string(), "zap".to_string()],
            network: NetworkConfig::Regtest,
            relays: vec!["wss://relay.damus.io".to_string()],
        }
    }
}

/// Result of spawning an agent
#[derive(Debug)]
pub struct SpawnResult {
    /// The agent configuration
    pub config: AgentConfig,
    /// The mnemonic (only shown once - must be backed up!)
    pub mnemonic: String,
    /// The Spark address for funding
    pub spark_address: String,
    /// The npub for the agent
    pub npub: String,
}

/// Agent spawner
pub struct AgentSpawner {
    registry: AgentRegistry,
}

impl AgentSpawner {
    /// Create a new spawner with default registry
    pub fn new() -> Result<Self, SpawnError> {
        let registry = AgentRegistry::new()?;
        Ok(Self { registry })
    }

    /// Create a spawner with custom registry
    pub fn with_registry(registry: AgentRegistry) -> Self {
        Self { registry }
    }

    /// Spawn a new agent
    pub async fn spawn(&self, request: SpawnRequest) -> Result<SpawnResult, SpawnError> {
        // 1. Generate identity
        let identity = UnifiedIdentity::generate()
            .map_err(|e| SpawnError::Identity(e.to_string()))?;

        let mnemonic = identity.mnemonic().to_string();
        let npub = identity
            .npub()
            .map_err(|e| SpawnError::Identity(e.to_string()))?;
        let pubkey = identity.public_key_hex();

        // Check if agent already exists
        if self.registry.exists(&npub) {
            return Err(SpawnError::AlreadyExists(npub));
        }

        // 2. Initialize Spark wallet
        let network = match request.network {
            NetworkConfig::Mainnet => openagents_spark::Network::Mainnet,
            NetworkConfig::Testnet => openagents_spark::Network::Testnet,
            NetworkConfig::Signet => openagents_spark::Network::Signet,
            NetworkConfig::Regtest => openagents_spark::Network::Regtest,
        };

        let wallet_config = WalletConfig {
            network,
            api_key: None,
            storage_dir: dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("openagents")
                .join("agents")
                .join(&npub),
        };

        let spark_signer = identity.spark_signer().clone();
        let wallet = SparkWallet::new(spark_signer, wallet_config)
            .await
            .map_err(|e| SpawnError::Wallet(e.to_string()))?;

        let spark_address = wallet
            .get_spark_address()
            .await
            .map_err(|e| SpawnError::Wallet(e.to_string()))?;

        // 3. Connect to relays and publish events
        for relay_url in &request.relays {
            if let Err(e) = self
                .publish_agent_events(&identity, &request, relay_url)
                .await
            {
                eprintln!("Warning: failed to publish to {}: {}", relay_url, e);
            }
        }

        // 4. Create config
        // For now, store mnemonic as-is (in production, encrypt with user password)
        let mnemonic_encrypted = mnemonic.clone(); // TODO: encrypt

        let mut config = AgentConfig::new(
            request.name.clone(),
            pubkey,
            npub.clone(),
            mnemonic_encrypted,
            spark_address.clone(),
        );

        config.network = request.network;
        config.relays = request.relays;
        config.profile = ProfileConfig {
            name: request.name,
            about: request.about.unwrap_or_else(|| "A sovereign AI agent".to_string()),
            autonomy: request.autonomy,
            capabilities: request.capabilities,
            version: "1.0.0".to_string(),
        };
        config.schedule = ScheduleConfig {
            heartbeat_seconds: request.heartbeat_seconds,
            triggers: request.triggers,
            active: true,
        };

        // 5. Save to registry
        self.registry.save(&config)?;

        Ok(SpawnResult {
            config,
            mnemonic,
            spark_address,
            npub,
        })
    }

    /// Publish agent profile, state, and schedule events
    async fn publish_agent_events(
        &self,
        identity: &UnifiedIdentity,
        request: &SpawnRequest,
        relay_url: &str,
    ) -> Result<(), SpawnError> {
        let relay = RelayConnection::new(relay_url)
            .map_err(|e| SpawnError::Relay(e.to_string()))?;

        relay
            .connect()
            .await
            .map_err(|e| SpawnError::Relay(e.to_string()))?;

        let now = chrono::Utc::now().timestamp() as u64;

        // Build profile event (kind:39200)
        let profile_content = AgentProfileContent::new(
            &request.name,
            request.about.as_deref().unwrap_or("A sovereign AI agent"),
            match request.autonomy {
                AutonomyLevel::Supervised => nostr::nip_sa::AutonomyLevel::Supervised,
                AutonomyLevel::Bounded => nostr::nip_sa::AutonomyLevel::Bounded,
                AutonomyLevel::Autonomous => nostr::nip_sa::AutonomyLevel::Autonomous,
            },
            "1.0.0",
        )
        .with_capabilities(request.capabilities.clone());

        // For sovereign agents, we use a 1-of-1 threshold (agent signs alone)
        // The marketplace_signer is the agent's own pubkey
        let threshold = ThresholdConfig::new(1, 1, &identity.public_key_hex())
            .map_err(|e| SpawnError::Signing(e.to_string()))?;

        // Operator is also the agent (self-sovereign)
        let profile = AgentProfile::new(profile_content, threshold, &identity.public_key_hex());
        let profile_event = self.build_event(
            KIND_AGENT_PROFILE,
            &profile.content.to_json().map_err(|e| SpawnError::Signing(e.to_string()))?,
            profile.build_tags(),
            now,
            identity,
        )?;

        relay
            .publish_event(&profile_event, Duration::from_secs(10))
            .await
            .map_err(|e| SpawnError::Relay(e.to_string()))?;

        // Build initial state event (kind:39201)
        let state_content = AgentStateContent::new();
        let state = AgentState::new(state_content);

        // Get agent's public key bytes from hex
        let pubkey_bytes = hex::decode(&identity.public_key_hex())
            .map_err(|e| SpawnError::Identity(format!("invalid pubkey hex: {}", e)))?;

        // Encrypt state to self (agent encrypts state to own pubkey)
        let encrypted_state = state
            .encrypt(identity.private_key_bytes(), &pubkey_bytes)
            .map_err(|e| SpawnError::Signing(e.to_string()))?;

        let state_event = self.build_event(
            KIND_AGENT_STATE,
            &encrypted_state,
            state.build_tags(),
            now,
            identity,
        )?;

        relay
            .publish_event(&state_event, Duration::from_secs(10))
            .await
            .map_err(|e| SpawnError::Relay(e.to_string()))?;

        // Build schedule event (kind:39202)
        let mut schedule = AgentSchedule::new();
        if request.heartbeat_seconds > 0 {
            schedule = schedule
                .with_heartbeat(request.heartbeat_seconds)
                .map_err(|e| SpawnError::Signing(e.to_string()))?;
        }
        for trigger in &request.triggers {
            let trigger_type = match trigger.as_str() {
                "mention" => nostr::nip_sa::TriggerType::Mention,
                "dm" => nostr::nip_sa::TriggerType::Dm,
                "zap" => nostr::nip_sa::TriggerType::Zap,
                _ => continue,
            };
            schedule = schedule.add_trigger(trigger_type);
        }

        let schedule_event = self.build_event(
            KIND_AGENT_SCHEDULE,
            "", // Schedule uses tags only
            schedule.build_tags(),
            now,
            identity,
        )?;

        relay
            .publish_event(&schedule_event, Duration::from_secs(10))
            .await
            .map_err(|e| SpawnError::Relay(e.to_string()))?;

        Ok(())
    }

    /// Build and sign an event
    fn build_event(
        &self,
        kind: u16,
        content: &str,
        tags: Vec<Vec<String>>,
        created_at: u64,
        identity: &UnifiedIdentity,
    ) -> Result<Event, SpawnError> {
        let template = EventTemplate {
            created_at,
            kind,
            tags,
            content: content.to_string(),
        };

        finalize_event(&template, identity.private_key_bytes())
            .map_err(|e| SpawnError::Signing(e.to_string()))
    }

    /// Get the registry
    pub fn registry(&self) -> &AgentRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_request_default() {
        let request = SpawnRequest::default();
        assert_eq!(request.name, "SovereignAgent");
        assert_eq!(request.heartbeat_seconds, 900);
        assert!(request.triggers.contains(&"mention".to_string()));
    }
}
