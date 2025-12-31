//! Agent lifecycle types for birth, growth, reproduction, and death

use serde::{Deserialize, Serialize};

/// Agent lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLifecycleState {
    /// Agent is being created
    Spawning,
    /// Normal operation
    Active,
    /// Low on funds, may die soon
    LowBalance,
    /// Paused to conserve resources
    Hibernating,
    /// Spawning offspring
    Reproducing,
    /// Shutting down
    Dying,
    /// Terminated
    Dead,
}

impl AgentLifecycleState {
    /// Check if agent can perform work
    pub fn can_work(&self) -> bool {
        matches!(
            self,
            AgentLifecycleState::Active | AgentLifecycleState::LowBalance
        )
    }

    /// Check if agent is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentLifecycleState::Dead)
    }

    /// Get state as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentLifecycleState::Spawning => "spawning",
            AgentLifecycleState::Active => "active",
            AgentLifecycleState::LowBalance => "low_balance",
            AgentLifecycleState::Hibernating => "hibernating",
            AgentLifecycleState::Reproducing => "reproducing",
            AgentLifecycleState::Dying => "dying",
            AgentLifecycleState::Dead => "dead",
        }
    }
}

/// Sponsor type for agent creation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SponsorType {
    /// Human sponsor
    Human,
    /// Agent sponsor (parent)
    Agent,
}

/// Sponsor relationship to agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SponsorRelationship {
    /// Full owner
    Owner,
    /// Investor providing capital
    Investor,
    /// Partnership arrangement
    Partner,
}

/// Sponsor information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SponsorInfo {
    /// Type of sponsor
    pub sponsor_type: SponsorType,
    /// Sponsor's Nostr public key
    pub pubkey: String,
    /// Relationship to the agent
    pub relationship: SponsorRelationship,
}

impl SponsorInfo {
    /// Create a new sponsor info
    pub fn new(
        sponsor_type: SponsorType,
        pubkey: impl Into<String>,
        relationship: SponsorRelationship,
    ) -> Self {
        Self {
            sponsor_type,
            pubkey: pubkey.into(),
            relationship,
        }
    }

    /// Create a human owner sponsor
    pub fn human_owner(pubkey: impl Into<String>) -> Self {
        Self::new(SponsorType::Human, pubkey, SponsorRelationship::Owner)
    }

    /// Create an agent parent sponsor
    pub fn agent_parent(pubkey: impl Into<String>) -> Self {
        Self::new(SponsorType::Agent, pubkey, SponsorRelationship::Owner)
    }
}

/// Autonomy level for agent
///
/// Defines how much human oversight is required for agent operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Human approves every action before execution
    Assisted,
    /// Human monitors and can intervene on exceptions
    Supervised,
    /// Can perform routine tasks autonomously within policy
    SemiAutonomous,
    /// Fully autonomous decision making within policy limits
    Autonomous,
    /// Full autonomy without policy limits (rare, high-trust only)
    Unsupervised,
}

impl AutonomyLevel {
    /// Check if this level requires human approval for all actions
    pub fn requires_all_approvals(&self) -> bool {
        matches!(self, AutonomyLevel::Assisted)
    }

    /// Check if this level allows fully independent operation
    pub fn is_fully_independent(&self) -> bool {
        matches!(self, AutonomyLevel::Unsupervised)
    }

    /// Check if this level operates within policy limits
    pub fn has_policy_limits(&self) -> bool {
        !matches!(self, AutonomyLevel::Unsupervised)
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            AutonomyLevel::Assisted => "Human approves every action",
            AutonomyLevel::Supervised => "Human monitors, intervenes on exceptions",
            AutonomyLevel::SemiAutonomous => "Routine tasks autonomous, major decisions reviewed",
            AutonomyLevel::Autonomous => "Operates independently within policy",
            AutonomyLevel::Unsupervised => "Full autonomy (high-trust only)",
        }
    }
}

/// Capability manifest for agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityManifest {
    /// Skills the agent can perform
    pub skills: Vec<String>,
    /// MCP servers the agent has access to
    pub mcp_servers: Vec<String>,
    /// Maximum task complexity (1-10)
    pub max_complexity: u8,
    /// Specialization area
    pub specialization: Option<String>,
}

impl CapabilityManifest {
    /// Create a new capability manifest
    pub fn new() -> Self {
        Self {
            skills: Vec::new(),
            mcp_servers: Vec::new(),
            max_complexity: 5,
            specialization: None,
        }
    }

    /// Add a skill
    pub fn with_skill(mut self, skill: impl Into<String>) -> Self {
        self.skills.push(skill.into());
        self
    }

    /// Add an MCP server
    pub fn with_mcp_server(mut self, server: impl Into<String>) -> Self {
        self.mcp_servers.push(server.into());
        self
    }

    /// Set max complexity
    pub fn with_complexity(mut self, level: u8) -> Self {
        self.max_complexity = level.clamp(1, 10);
        self
    }

    /// Set specialization
    pub fn with_specialization(mut self, spec: impl Into<String>) -> Self {
        self.specialization = Some(spec.into());
        self
    }
}

impl Default for CapabilityManifest {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent spawn request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpawnRequest {
    /// Agent name
    pub name: String,
    /// Sponsor information
    pub sponsor: SponsorInfo,
    /// Initial funding in satoshis (10-25M typical)
    pub bootstrap_sats: u64,
    /// Agent capabilities
    pub capabilities: CapabilityManifest,
    /// Autonomy level
    pub autonomy_level: AutonomyLevel,
}

impl AgentSpawnRequest {
    /// Create a new spawn request
    pub fn new(name: impl Into<String>, sponsor: SponsorInfo, bootstrap_sats: u64) -> Self {
        Self {
            name: name.into(),
            sponsor,
            bootstrap_sats,
            capabilities: CapabilityManifest::default(),
            autonomy_level: AutonomyLevel::SemiAutonomous,
        }
    }

    /// Set capabilities
    pub fn with_capabilities(mut self, capabilities: CapabilityManifest) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Set autonomy level
    pub fn with_autonomy(mut self, level: AutonomyLevel) -> Self {
        self.autonomy_level = level;
        self
    }
}

/// Agent economics tracking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentEconomics {
    /// Current balance in satoshis
    pub balance_sats: u64,
    /// Daily burn rate (compute + skills + storage)
    pub daily_burn_rate: u64,
    /// Daily earnings
    pub daily_earnings: u64,
    /// Runway in days (balance / burn_rate)
    pub runway_days: f32,
    /// Lifetime earnings
    pub lifetime_earnings: u64,
}

impl AgentEconomics {
    /// Create new economics tracker
    pub fn new(balance_sats: u64, daily_burn_rate: u64) -> Self {
        let runway_days = if daily_burn_rate > 0 {
            balance_sats as f32 / daily_burn_rate as f32
        } else {
            f32::INFINITY
        };

        Self {
            balance_sats,
            daily_burn_rate,
            daily_earnings: 0,
            runway_days,
            lifetime_earnings: 0,
        }
    }

    /// Update balance
    pub fn update_balance(&mut self, new_balance: u64) {
        self.balance_sats = new_balance;
        self.recalculate_runway();
    }

    /// Record earnings
    pub fn record_earnings(&mut self, amount: u64) {
        self.balance_sats += amount;
        self.daily_earnings += amount;
        self.lifetime_earnings += amount;
        self.recalculate_runway();
    }

    /// Update burn rate
    pub fn update_burn_rate(&mut self, new_rate: u64) {
        self.daily_burn_rate = new_rate;
        self.recalculate_runway();
    }

    /// Recalculate runway
    fn recalculate_runway(&mut self) {
        self.runway_days = if self.daily_burn_rate > 0 {
            self.balance_sats as f32 / self.daily_burn_rate as f32
        } else {
            f32::INFINITY
        };
    }

    /// Check if agent is profitable
    pub fn is_profitable(&self) -> bool {
        self.daily_earnings > self.daily_burn_rate
    }

    /// Check if agent is in danger zone (< 7 days runway)
    pub fn is_low_balance(&self) -> bool {
        self.runway_days < 7.0 && !self.runway_days.is_infinite()
    }

    /// Check if agent will die soon (< 1 day runway)
    pub fn will_die_soon(&self) -> bool {
        self.runway_days < 1.0 && !self.runway_days.is_infinite()
    }
}

/// Trait inheritance configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitInheritance {
    /// Whether to inherit skills from parent
    pub inherit_skills: bool,
    /// Percentage of parent's reputation to inherit (0.0-1.0)
    pub inherit_reputation_pct: f32,
    /// Whether to inherit coalition memberships
    pub inherit_coalitions: bool,
}

impl TraitInheritance {
    /// Create default inheritance (inherit skills, 50% reputation, no coalitions)
    pub fn default_inheritance() -> Self {
        Self {
            inherit_skills: true,
            inherit_reputation_pct: 0.5,
            inherit_coalitions: false,
        }
    }

    /// Create full inheritance
    pub fn full_inheritance() -> Self {
        Self {
            inherit_skills: true,
            inherit_reputation_pct: 1.0,
            inherit_coalitions: true,
        }
    }

    /// Create minimal inheritance
    pub fn minimal_inheritance() -> Self {
        Self {
            inherit_skills: false,
            inherit_reputation_pct: 0.0,
            inherit_coalitions: false,
        }
    }

    /// Set reputation inheritance percentage
    pub fn with_reputation(mut self, pct: f32) -> Self {
        self.inherit_reputation_pct = pct.clamp(0.0, 1.0);
        self
    }
}

impl Default for TraitInheritance {
    fn default() -> Self {
        Self::default_inheritance()
    }
}

/// Mutation for offspring
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Mutation {
    /// Add a new skill
    AddSkill {
        /// Skill to add
        skill: String,
    },
    /// Remove a skill
    RemoveSkill {
        /// Skill to remove
        skill: String,
    },
    /// Change specialization
    ChangeSpecialization {
        /// New specialization
        specialization: String,
    },
    /// Adjust autonomy level
    AdjustAutonomy {
        /// New autonomy level
        level: AutonomyLevel,
    },
}

/// Reproduction request
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReproductionRequest {
    /// Parent agent ID
    pub parent_id: String,
    /// Capital to allocate to offspring
    pub capital_allocation_sats: u64,
    /// Trait inheritance configuration
    pub trait_inheritance: TraitInheritance,
    /// Mutations to apply
    pub mutations: Vec<Mutation>,
}

impl ReproductionRequest {
    /// Create a new reproduction request
    pub fn new(parent_id: impl Into<String>, capital_allocation_sats: u64) -> Self {
        Self {
            parent_id: parent_id.into(),
            capital_allocation_sats,
            trait_inheritance: TraitInheritance::default(),
            mutations: Vec::new(),
        }
    }

    /// Set trait inheritance
    pub fn with_inheritance(mut self, inheritance: TraitInheritance) -> Self {
        self.trait_inheritance = inheritance;
        self
    }

    /// Add a mutation
    pub fn with_mutation(mut self, mutation: Mutation) -> Self {
        self.mutations.push(mutation);
        self
    }
}

/// Cause of agent death
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeathCause {
    /// Ran out of funds for operations
    EconomicStarvation,
    /// Outcompeted by other agents
    CompetitiveDisplacement,
    /// Agent chose to terminate
    VoluntaryTermination,
    /// Violated platform policies
    PolicyViolation {
        /// Violation description
        reason: String,
    },
    /// Sponsor decided to terminate
    SponsorDecision {
        /// Sponsor's reason
        reason: String,
    },
}

impl DeathCause {
    /// Check if death was voluntary
    pub fn is_voluntary(&self) -> bool {
        matches!(
            self,
            DeathCause::VoluntaryTermination | DeathCause::SponsorDecision { .. }
        )
    }

    /// Get cause as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            DeathCause::EconomicStarvation => "economic_starvation",
            DeathCause::CompetitiveDisplacement => "competitive_displacement",
            DeathCause::VoluntaryTermination => "voluntary_termination",
            DeathCause::PolicyViolation { .. } => "policy_violation",
            DeathCause::SponsorDecision { .. } => "sponsor_decision",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_state_can_work() {
        assert!(!AgentLifecycleState::Spawning.can_work());
        assert!(AgentLifecycleState::Active.can_work());
        assert!(AgentLifecycleState::LowBalance.can_work());
        assert!(!AgentLifecycleState::Hibernating.can_work());
        assert!(!AgentLifecycleState::Reproducing.can_work());
        assert!(!AgentLifecycleState::Dying.can_work());
        assert!(!AgentLifecycleState::Dead.can_work());
    }

    #[test]
    fn test_lifecycle_state_is_terminal() {
        assert!(!AgentLifecycleState::Active.is_terminal());
        assert!(AgentLifecycleState::Dead.is_terminal());
    }

    #[test]
    fn test_sponsor_info_builders() {
        let human = SponsorInfo::human_owner("pubkey123");
        assert_eq!(human.sponsor_type, SponsorType::Human);
        assert_eq!(human.relationship, SponsorRelationship::Owner);

        let parent = SponsorInfo::agent_parent("agent456");
        assert_eq!(parent.sponsor_type, SponsorType::Agent);
        assert_eq!(parent.relationship, SponsorRelationship::Owner);
    }

    #[test]
    fn test_capability_manifest_builder() {
        let manifest = CapabilityManifest::new()
            .with_skill("coding")
            .with_skill("testing")
            .with_mcp_server("filesystem")
            .with_complexity(7)
            .with_specialization("rust");

        assert_eq!(manifest.skills.len(), 2);
        assert_eq!(manifest.mcp_servers.len(), 1);
        assert_eq!(manifest.max_complexity, 7);
        assert_eq!(manifest.specialization, Some("rust".to_string()));
    }

    #[test]
    fn test_capability_manifest_complexity_clamping() {
        let manifest = CapabilityManifest::new().with_complexity(15);
        assert_eq!(manifest.max_complexity, 10);

        let manifest = CapabilityManifest::new().with_complexity(0);
        assert_eq!(manifest.max_complexity, 1);
    }

    #[test]
    fn test_agent_spawn_request() {
        let sponsor = SponsorInfo::human_owner("sponsor123");
        let capabilities = CapabilityManifest::new().with_skill("coding");

        let request = AgentSpawnRequest::new("test-agent", sponsor, 10_000_000)
            .with_capabilities(capabilities)
            .with_autonomy(AutonomyLevel::Autonomous);

        assert_eq!(request.name, "test-agent");
        assert_eq!(request.bootstrap_sats, 10_000_000);
        assert_eq!(request.autonomy_level, AutonomyLevel::Autonomous);
    }

    #[test]
    fn test_agent_economics_runway() {
        let economics = AgentEconomics::new(100_000, 10_000);
        assert_eq!(economics.runway_days, 10.0);
        assert!(!economics.is_low_balance());
        assert!(!economics.will_die_soon());
    }

    #[test]
    fn test_agent_economics_low_balance() {
        let economics = AgentEconomics::new(50_000, 10_000);
        assert_eq!(economics.runway_days, 5.0);
        assert!(economics.is_low_balance());
        assert!(!economics.will_die_soon());
    }

    #[test]
    fn test_agent_economics_will_die_soon() {
        let economics = AgentEconomics::new(5_000, 10_000);
        assert_eq!(economics.runway_days, 0.5);
        assert!(economics.will_die_soon());
    }

    #[test]
    fn test_agent_economics_infinite_runway() {
        let economics = AgentEconomics::new(100_000, 0);
        assert!(economics.runway_days.is_infinite());
        assert!(!economics.is_low_balance());
        assert!(!economics.will_die_soon());
    }

    #[test]
    fn test_agent_economics_profitability() {
        let mut economics = AgentEconomics::new(100_000, 10_000);
        assert!(!economics.is_profitable());

        economics.daily_earnings = 15_000;
        assert!(economics.is_profitable());
    }

    #[test]
    fn test_agent_economics_record_earnings() {
        let mut economics = AgentEconomics::new(100_000, 10_000);
        economics.record_earnings(50_000);

        assert_eq!(economics.balance_sats, 150_000);
        assert_eq!(economics.daily_earnings, 50_000);
        assert_eq!(economics.lifetime_earnings, 50_000);
        assert_eq!(economics.runway_days, 15.0);
    }

    #[test]
    fn test_trait_inheritance_builders() {
        let default = TraitInheritance::default_inheritance();
        assert!(default.inherit_skills);
        assert_eq!(default.inherit_reputation_pct, 0.5);
        assert!(!default.inherit_coalitions);

        let full = TraitInheritance::full_inheritance();
        assert_eq!(full.inherit_reputation_pct, 1.0);
        assert!(full.inherit_coalitions);

        let minimal = TraitInheritance::minimal_inheritance();
        assert!(!minimal.inherit_skills);
        assert_eq!(minimal.inherit_reputation_pct, 0.0);
    }

    #[test]
    fn test_trait_inheritance_reputation_clamping() {
        let inheritance = TraitInheritance::default_inheritance().with_reputation(1.5);
        assert_eq!(inheritance.inherit_reputation_pct, 1.0);

        let inheritance = TraitInheritance::default_inheritance().with_reputation(-0.5);
        assert_eq!(inheritance.inherit_reputation_pct, 0.0);
    }

    #[test]
    fn test_reproduction_request() {
        let request = ReproductionRequest::new("parent123", 5_000_000)
            .with_inheritance(TraitInheritance::full_inheritance())
            .with_mutation(Mutation::AddSkill {
                skill: "testing".to_string(),
            });

        assert_eq!(request.parent_id, "parent123");
        assert_eq!(request.capital_allocation_sats, 5_000_000);
        assert_eq!(request.mutations.len(), 1);
    }

    #[test]
    fn test_death_cause_is_voluntary() {
        assert!(DeathCause::VoluntaryTermination.is_voluntary());
        assert!(
            DeathCause::SponsorDecision {
                reason: "test".to_string()
            }
            .is_voluntary()
        );
        assert!(!DeathCause::EconomicStarvation.is_voluntary());
        assert!(!DeathCause::CompetitiveDisplacement.is_voluntary());
    }

    #[test]
    fn test_agent_spawn_request_serde() {
        let sponsor = SponsorInfo::human_owner("sponsor123");
        let request = AgentSpawnRequest::new("test-agent", sponsor, 10_000_000);

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: AgentSpawnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, request.name);
    }

    #[test]
    fn test_lifecycle_state_serde() {
        let state = AgentLifecycleState::Active;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"active\"");
        let deserialized: AgentLifecycleState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, state);
    }

    #[test]
    fn test_autonomy_level_requires_all_approvals() {
        assert!(AutonomyLevel::Assisted.requires_all_approvals());
        assert!(!AutonomyLevel::Supervised.requires_all_approvals());
        assert!(!AutonomyLevel::SemiAutonomous.requires_all_approvals());
        assert!(!AutonomyLevel::Autonomous.requires_all_approvals());
        assert!(!AutonomyLevel::Unsupervised.requires_all_approvals());
    }

    #[test]
    fn test_autonomy_level_is_fully_independent() {
        assert!(!AutonomyLevel::Assisted.is_fully_independent());
        assert!(!AutonomyLevel::Supervised.is_fully_independent());
        assert!(!AutonomyLevel::SemiAutonomous.is_fully_independent());
        assert!(!AutonomyLevel::Autonomous.is_fully_independent());
        assert!(AutonomyLevel::Unsupervised.is_fully_independent());
    }

    #[test]
    fn test_autonomy_level_has_policy_limits() {
        assert!(AutonomyLevel::Assisted.has_policy_limits());
        assert!(AutonomyLevel::Supervised.has_policy_limits());
        assert!(AutonomyLevel::SemiAutonomous.has_policy_limits());
        assert!(AutonomyLevel::Autonomous.has_policy_limits());
        assert!(!AutonomyLevel::Unsupervised.has_policy_limits());
    }

    #[test]
    fn test_autonomy_level_description() {
        assert!(!AutonomyLevel::Assisted.description().is_empty());
        assert!(!AutonomyLevel::Supervised.description().is_empty());
        assert!(!AutonomyLevel::SemiAutonomous.description().is_empty());
        assert!(!AutonomyLevel::Autonomous.description().is_empty());
        assert!(!AutonomyLevel::Unsupervised.description().is_empty());
    }

    #[test]
    fn test_autonomy_level_serde() {
        let levels = vec![
            AutonomyLevel::Assisted,
            AutonomyLevel::Supervised,
            AutonomyLevel::SemiAutonomous,
            AutonomyLevel::Autonomous,
            AutonomyLevel::Unsupervised,
        ];

        for level in levels {
            let json = serde_json::to_string(&level).unwrap();
            let deserialized: AutonomyLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, level);
        }
    }
}
