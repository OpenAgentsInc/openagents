//! Types for agent coalitions and collaborative work

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of coalition structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoalitionType {
    /// Single task coalition that dissolves after completion
    AdHoc,
    /// Persistent team for ongoing collaboration
    Standing,
    /// Competitive bidding market for tasks
    Market,
    /// Hierarchical structure with coordinator and specialists
    Hierarchical,
}

impl CoalitionType {
    /// Get coalition type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            CoalitionType::AdHoc => "ad_hoc",
            CoalitionType::Standing => "standing",
            CoalitionType::Market => "market",
            CoalitionType::Hierarchical => "hierarchical",
        }
    }
}

/// Coalition status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoalitionStatus {
    /// Coalition is forming, waiting for members
    Forming,
    /// Coalition is actively working on task
    Active,
    /// Task is complete, awaiting payment settlement
    Completed,
    /// Payment has been distributed
    Settled,
    /// Coalition was dissolved before completion
    Dissolved,
}

impl CoalitionStatus {
    /// Check if coalition can accept new members
    pub fn can_accept_members(&self) -> bool {
        matches!(self, CoalitionStatus::Forming | CoalitionStatus::Active)
    }

    /// Check if coalition is operational
    pub fn is_operational(&self) -> bool {
        matches!(self, CoalitionStatus::Active)
    }

    /// Check if coalition is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, CoalitionStatus::Settled | CoalitionStatus::Dissolved)
    }

    /// Get status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            CoalitionStatus::Forming => "forming",
            CoalitionStatus::Active => "active",
            CoalitionStatus::Completed => "completed",
            CoalitionStatus::Settled => "settled",
            CoalitionStatus::Dissolved => "dissolved",
        }
    }
}

/// Member of a coalition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoalitionMember {
    /// Agent ID
    pub agent_id: String,
    /// Role in the coalition
    pub role: String,
    /// Contribution weight (0.0-1.0)
    pub contribution_weight: f32,
    /// When this member joined
    pub joined_at: DateTime<Utc>,
}

impl CoalitionMember {
    /// Create a new coalition member
    pub fn new(agent_id: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            role: role.into(),
            contribution_weight: 0.0,
            joined_at: Utc::now(),
        }
    }

    /// Set contribution weight
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.contribution_weight = weight.clamp(0.0, 1.0);
        self
    }
}

/// Work contribution to a coalition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    /// Agent who contributed
    pub agent_id: String,
    /// Type of work performed
    pub work_type: String,
    /// Contribution weight (0.0-1.0)
    pub weight: f32,
    /// Receipt/proof of work
    pub receipts: Vec<String>,
}

impl Contribution {
    /// Create a new contribution
    pub fn new(agent_id: impl Into<String>, work_type: impl Into<String>, weight: f32) -> Self {
        Self {
            agent_id: agent_id.into(),
            work_type: work_type.into(),
            weight: weight.clamp(0.0, 1.0),
            receipts: Vec::new(),
        }
    }

    /// Add a receipt
    pub fn with_receipt(mut self, receipt: impl Into<String>) -> Self {
        self.receipts.push(receipt.into());
        self
    }
}

/// Payment pool for a coalition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentPool {
    /// Total satoshis in the pool
    pub total_sats: u64,
    /// Contributions from members
    pub contributions: Vec<Contribution>,
    /// Whether the pool has been settled
    pub settled: bool,
}

impl PaymentPool {
    /// Create a new payment pool
    pub fn new(total_sats: u64) -> Self {
        Self {
            total_sats,
            contributions: Vec::new(),
            settled: false,
        }
    }

    /// Add a contribution
    pub fn add_contribution(&mut self, contribution: Contribution) {
        self.contributions.push(contribution);
    }

    /// Calculate total contribution weight
    pub fn total_weight(&self) -> f32 {
        self.contributions.iter().map(|c| c.weight).sum()
    }

    /// Settle the pool and return payment splits
    pub fn settle(&mut self) -> Vec<PaymentSplit> {
        if self.settled {
            return Vec::new();
        }

        let total_weight = self.total_weight();
        if total_weight == 0.0 {
            return Vec::new();
        }

        let mut splits = Vec::new();
        let mut distributed = 0u64;

        // Calculate each agent's share
        for contribution in &self.contributions {
            let share = ((self.total_sats as f64 * contribution.weight as f64)
                / total_weight as f64) as u64;
            distributed += share;

            splits.push(PaymentSplit {
                agent_id: contribution.agent_id.clone(),
                amount_sats: share,
                weight: contribution.weight,
            });
        }

        // Handle rounding remainder - give to first contributor
        let remainder = self.total_sats.saturating_sub(distributed);
        if remainder > 0 && !splits.is_empty() {
            splits[0].amount_sats += remainder;
        }

        self.settled = true;
        splits
    }
}

/// Payment split for a coalition member
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentSplit {
    /// Agent receiving payment
    pub agent_id: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Contribution weight
    pub weight: f32,
}

/// Coalition for collaborative agent work
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coalition {
    /// Unique coalition ID
    pub id: String,
    /// Coalition type
    pub coalition_type: CoalitionType,
    /// Members of the coalition
    pub members: Vec<CoalitionMember>,
    /// Task being worked on
    pub task: Option<String>,
    /// Current status
    pub status: CoalitionStatus,
    /// Payment pool
    pub payment_pool: PaymentPool,
    /// When coalition was created
    pub created_at: DateTime<Utc>,
}

impl Coalition {
    /// Create a new coalition
    pub fn new(
        id: impl Into<String>,
        coalition_type: CoalitionType,
        payment_pool: PaymentPool,
    ) -> Self {
        Self {
            id: id.into(),
            coalition_type,
            members: Vec::new(),
            task: None,
            status: CoalitionStatus::Forming,
            payment_pool,
            created_at: Utc::now(),
        }
    }

    /// Set task
    pub fn with_task(mut self, task: impl Into<String>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Add a member
    pub fn add_member(&mut self, member: CoalitionMember) -> Result<(), String> {
        if !self.status.can_accept_members() {
            return Err(format!(
                "Coalition is {} and cannot accept new members",
                self.status.as_str()
            ));
        }

        // Check if member already exists
        if self.members.iter().any(|m| m.agent_id == member.agent_id) {
            return Err("Agent is already a member of this coalition".to_string());
        }

        self.members.push(member);
        Ok(())
    }

    /// Remove a member
    pub fn remove_member(&mut self, agent_id: &str) -> Result<(), String> {
        if self.status.is_terminal() {
            return Err("Cannot remove members from a terminal coalition".to_string());
        }

        let initial_len = self.members.len();
        self.members.retain(|m| m.agent_id != agent_id);

        if self.members.len() == initial_len {
            return Err("Agent is not a member of this coalition".to_string());
        }

        Ok(())
    }

    /// Activate coalition
    pub fn activate(&mut self) -> Result<(), String> {
        if self.status != CoalitionStatus::Forming {
            return Err(format!(
                "Cannot activate coalition from {} state",
                self.status.as_str()
            ));
        }

        if self.members.is_empty() {
            return Err("Cannot activate coalition with no members".to_string());
        }

        self.status = CoalitionStatus::Active;
        Ok(())
    }

    /// Complete the coalition task
    pub fn complete(&mut self) -> Result<(), String> {
        if self.status != CoalitionStatus::Active {
            return Err(format!(
                "Cannot complete coalition from {} state",
                self.status.as_str()
            ));
        }

        self.status = CoalitionStatus::Completed;
        Ok(())
    }

    /// Settle payments
    pub fn settle(&mut self) -> Result<Vec<PaymentSplit>, String> {
        if self.status != CoalitionStatus::Completed {
            return Err(format!(
                "Cannot settle coalition from {} state",
                self.status.as_str()
            ));
        }

        let splits = self.payment_pool.settle();
        self.status = CoalitionStatus::Settled;
        Ok(splits)
    }

    /// Dissolve coalition without completing
    pub fn dissolve(&mut self) -> Result<(), String> {
        if self.status.is_terminal() {
            return Err("Coalition is already in a terminal state".to_string());
        }

        self.status = CoalitionStatus::Dissolved;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coalition_type_as_str() {
        assert_eq!(CoalitionType::AdHoc.as_str(), "ad_hoc");
        assert_eq!(CoalitionType::Standing.as_str(), "standing");
        assert_eq!(CoalitionType::Market.as_str(), "market");
        assert_eq!(CoalitionType::Hierarchical.as_str(), "hierarchical");
    }

    #[test]
    fn test_coalition_status_can_accept_members() {
        assert!(CoalitionStatus::Forming.can_accept_members());
        assert!(CoalitionStatus::Active.can_accept_members());
        assert!(!CoalitionStatus::Completed.can_accept_members());
        assert!(!CoalitionStatus::Settled.can_accept_members());
        assert!(!CoalitionStatus::Dissolved.can_accept_members());
    }

    #[test]
    fn test_coalition_status_is_operational() {
        assert!(!CoalitionStatus::Forming.is_operational());
        assert!(CoalitionStatus::Active.is_operational());
        assert!(!CoalitionStatus::Completed.is_operational());
        assert!(!CoalitionStatus::Settled.is_operational());
    }

    #[test]
    fn test_coalition_status_is_terminal() {
        assert!(!CoalitionStatus::Forming.is_terminal());
        assert!(!CoalitionStatus::Active.is_terminal());
        assert!(!CoalitionStatus::Completed.is_terminal());
        assert!(CoalitionStatus::Settled.is_terminal());
        assert!(CoalitionStatus::Dissolved.is_terminal());
    }

    #[test]
    fn test_coalition_member_new() {
        let member = CoalitionMember::new("agent1", "developer");
        assert_eq!(member.agent_id, "agent1");
        assert_eq!(member.role, "developer");
        assert_eq!(member.contribution_weight, 0.0);
    }

    #[test]
    fn test_coalition_member_with_weight() {
        let member = CoalitionMember::new("agent1", "developer").with_weight(0.5);
        assert_eq!(member.contribution_weight, 0.5);

        // Test clamping
        let member_high = CoalitionMember::new("agent2", "tester").with_weight(1.5);
        assert_eq!(member_high.contribution_weight, 1.0);

        let member_low = CoalitionMember::new("agent3", "reviewer").with_weight(-0.5);
        assert_eq!(member_low.contribution_weight, 0.0);
    }

    #[test]
    fn test_contribution_new() {
        let contrib = Contribution::new("agent1", "coding", 0.6);
        assert_eq!(contrib.agent_id, "agent1");
        assert_eq!(contrib.work_type, "coding");
        assert_eq!(contrib.weight, 0.6);
        assert!(contrib.receipts.is_empty());
    }

    #[test]
    fn test_contribution_with_receipt() {
        let contrib = Contribution::new("agent1", "coding", 0.6)
            .with_receipt("receipt1")
            .with_receipt("receipt2");
        assert_eq!(contrib.receipts.len(), 2);
    }

    #[test]
    fn test_payment_pool_new() {
        let pool = PaymentPool::new(100_000);
        assert_eq!(pool.total_sats, 100_000);
        assert!(pool.contributions.is_empty());
        assert!(!pool.settled);
    }

    #[test]
    fn test_payment_pool_total_weight() {
        let mut pool = PaymentPool::new(100_000);
        pool.add_contribution(Contribution::new("agent1", "coding", 0.5));
        pool.add_contribution(Contribution::new("agent2", "testing", 0.3));

        assert_eq!(pool.total_weight(), 0.8);
    }

    #[test]
    fn test_payment_pool_settle() {
        let mut pool = PaymentPool::new(100_000);
        pool.add_contribution(Contribution::new("agent1", "coding", 0.6));
        pool.add_contribution(Contribution::new("agent2", "testing", 0.4));

        let splits = pool.settle();
        assert_eq!(splits.len(), 2);

        // 60% and 40% split
        assert_eq!(splits[0].agent_id, "agent1");
        assert_eq!(splits[1].agent_id, "agent2");

        // Total should equal pool
        let total_distributed: u64 = splits.iter().map(|s| s.amount_sats).sum();
        assert_eq!(total_distributed, 100_000);

        // Pool should be marked as settled
        assert!(pool.settled);

        // Second settle should return empty
        let splits2 = pool.settle();
        assert!(splits2.is_empty());
    }

    #[test]
    fn test_payment_pool_settle_with_remainder() {
        let mut pool = PaymentPool::new(100);
        pool.add_contribution(Contribution::new("agent1", "work", 1.0));
        pool.add_contribution(Contribution::new("agent2", "work", 1.0));
        pool.add_contribution(Contribution::new("agent3", "work", 1.0));

        let splits = pool.settle();

        // Each should get 33, but remainder goes to first
        let total: u64 = splits.iter().map(|s| s.amount_sats).sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn test_coalition_new() {
        let pool = PaymentPool::new(50_000);
        let coalition = Coalition::new("coalition1", CoalitionType::AdHoc, pool);

        assert_eq!(coalition.id, "coalition1");
        assert_eq!(coalition.coalition_type, CoalitionType::AdHoc);
        assert_eq!(coalition.status, CoalitionStatus::Forming);
        assert!(coalition.members.is_empty());
        assert!(coalition.task.is_none());
    }

    #[test]
    fn test_coalition_with_task() {
        let pool = PaymentPool::new(50_000);
        let coalition =
            Coalition::new("coalition1", CoalitionType::AdHoc, pool).with_task("Build feature X");

        assert_eq!(coalition.task, Some("Build feature X".to_string()));
    }

    #[test]
    fn test_coalition_add_member() {
        let pool = PaymentPool::new(50_000);
        let mut coalition = Coalition::new("coalition1", CoalitionType::AdHoc, pool);

        let member = CoalitionMember::new("agent1", "developer");
        assert!(coalition.add_member(member).is_ok());
        assert_eq!(coalition.members.len(), 1);

        // Adding duplicate should fail
        let member2 = CoalitionMember::new("agent1", "tester");
        assert!(coalition.add_member(member2).is_err());
    }

    #[test]
    fn test_coalition_remove_member() {
        let pool = PaymentPool::new(50_000);
        let mut coalition = Coalition::new("coalition1", CoalitionType::AdHoc, pool);

        coalition
            .add_member(CoalitionMember::new("agent1", "developer"))
            .unwrap();
        coalition
            .add_member(CoalitionMember::new("agent2", "tester"))
            .unwrap();

        assert!(coalition.remove_member("agent1").is_ok());
        assert_eq!(coalition.members.len(), 1);

        // Removing non-existent member should fail
        assert!(coalition.remove_member("agent3").is_err());
    }

    #[test]
    fn test_coalition_lifecycle() {
        let pool = PaymentPool::new(50_000);
        let mut coalition = Coalition::new("coalition1", CoalitionType::AdHoc, pool);

        // Start in Forming state
        assert_eq!(coalition.status, CoalitionStatus::Forming);

        // Cannot activate empty coalition
        assert!(coalition.activate().is_err());

        // Add members and activate
        coalition
            .add_member(CoalitionMember::new("agent1", "dev"))
            .unwrap();
        assert!(coalition.activate().is_ok());
        assert_eq!(coalition.status, CoalitionStatus::Active);

        // Complete the work
        assert!(coalition.complete().is_ok());
        assert_eq!(coalition.status, CoalitionStatus::Completed);

        // Cannot accept new members after completion
        let member = CoalitionMember::new("agent2", "tester");
        assert!(coalition.add_member(member).is_err());
    }

    #[test]
    fn test_coalition_settle() {
        let mut pool = PaymentPool::new(100_000);
        pool.add_contribution(Contribution::new("agent1", "coding", 0.7));
        pool.add_contribution(Contribution::new("agent2", "testing", 0.3));

        let mut coalition = Coalition::new("coalition1", CoalitionType::AdHoc, pool);
        coalition
            .add_member(CoalitionMember::new("agent1", "dev"))
            .unwrap();
        coalition
            .add_member(CoalitionMember::new("agent2", "tester"))
            .unwrap();
        coalition.activate().unwrap();
        coalition.complete().unwrap();

        let splits = coalition.settle().unwrap();
        assert_eq!(splits.len(), 2);
        assert_eq!(coalition.status, CoalitionStatus::Settled);

        // Total should equal pool
        let total: u64 = splits.iter().map(|s| s.amount_sats).sum();
        assert_eq!(total, 100_000);
    }

    #[test]
    fn test_coalition_dissolve() {
        let pool = PaymentPool::new(50_000);
        let mut coalition = Coalition::new("coalition1", CoalitionType::AdHoc, pool);
        coalition
            .add_member(CoalitionMember::new("agent1", "dev"))
            .unwrap();
        coalition.activate().unwrap();

        assert!(coalition.dissolve().is_ok());
        assert_eq!(coalition.status, CoalitionStatus::Dissolved);

        // Cannot dissolve again
        assert!(coalition.dissolve().is_err());
    }

    #[test]
    fn test_coalition_serde() {
        let pool = PaymentPool::new(50_000);
        let coalition =
            Coalition::new("coalition1", CoalitionType::Standing, pool).with_task("Build feature");

        let json = serde_json::to_string(&coalition).unwrap();
        let deserialized: Coalition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, coalition.id);
        assert_eq!(deserialized.coalition_type, coalition.coalition_type);
    }
}
