//! Agent-to-agent commerce types
//!
//! Implements types for agents hiring other agents, including hiring flows,
//! contracts, and coordinator patterns for delegating work.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Contract status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    /// Contract proposed but not yet accepted
    Proposed,
    /// Contract accepted by worker
    Accepted,
    /// Work in progress
    InProgress,
    /// Work completed
    Completed,
    /// Contract disputed
    Disputed,
    /// Contract cancelled
    Cancelled,
}

impl ContractStatus {
    /// Check if contract is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ContractStatus::Completed | ContractStatus::Disputed | ContractStatus::Cancelled
        )
    }

    /// Check if contract is active (worker can still complete it)
    pub fn is_active(&self) -> bool {
        matches!(self, ContractStatus::Accepted | ContractStatus::InProgress)
    }
}

/// Task specification for agent work
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Type of task (e.g., "code_generation", "data_analysis")
    pub task_type: String,
    /// Human-readable description
    pub description: String,
    /// Task inputs as JSON
    pub inputs: Value,
    /// Expected output format/description
    pub expected_output: String,
    /// Optional deadline
    pub deadline: Option<DateTime<Utc>>,
}

impl TaskSpec {
    /// Create a new task specification
    pub fn new(
        task_type: impl Into<String>,
        description: impl Into<String>,
        inputs: Value,
        expected_output: impl Into<String>,
    ) -> Self {
        Self {
            task_type: task_type.into(),
            description: description.into(),
            inputs,
            expected_output: expected_output.into(),
            deadline: None,
        }
    }

    /// Set a deadline
    pub fn with_deadline(mut self, deadline: DateTime<Utc>) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Check if task is past deadline
    pub fn is_past_deadline(&self) -> bool {
        if let Some(deadline) = self.deadline {
            Utc::now() > deadline
        } else {
            false
        }
    }
}

/// Requirements for hiring an agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HiringRequirements {
    /// Minimum reputation score required
    pub min_reputation: Option<f32>,
    /// Required skills
    pub required_skills: Vec<String>,
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: Option<u32>,
}

impl HiringRequirements {
    /// Create new hiring requirements
    pub fn new() -> Self {
        Self {
            min_reputation: None,
            required_skills: Vec::new(),
            max_latency_ms: None,
        }
    }

    /// Set minimum reputation
    pub fn with_min_reputation(mut self, min: f32) -> Self {
        self.min_reputation = Some(min);
        self
    }

    /// Add a required skill
    pub fn require_skill(mut self, skill: impl Into<String>) -> Self {
        self.required_skills.push(skill.into());
        self
    }

    /// Set maximum latency
    pub fn with_max_latency(mut self, ms: u32) -> Self {
        self.max_latency_ms = Some(ms);
        self
    }
}

impl Default for HiringRequirements {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to hire an agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HireAgentRequest {
    /// Agent doing the hiring
    pub hirer_id: String,
    /// Target agent to hire
    pub target_agent_id: String,
    /// Task specification
    pub task: TaskSpec,
    /// Budget in satoshis
    pub budget_sats: u64,
    /// Hiring requirements
    pub requirements: HiringRequirements,
}

impl HireAgentRequest {
    /// Create a new hire request
    pub fn new(
        hirer_id: impl Into<String>,
        target_agent_id: impl Into<String>,
        task: TaskSpec,
        budget_sats: u64,
    ) -> Self {
        Self {
            hirer_id: hirer_id.into(),
            target_agent_id: target_agent_id.into(),
            task,
            budget_sats,
            requirements: HiringRequirements::default(),
        }
    }

    /// Set requirements
    pub fn with_requirements(mut self, requirements: HiringRequirements) -> Self {
        self.requirements = requirements;
        self
    }
}

/// Contract between two agents
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentContract {
    /// Contract ID
    pub id: String,
    /// Hiring agent ID
    pub hirer: String,
    /// Working agent ID
    pub worker: String,
    /// Task specification
    pub task: TaskSpec,
    /// Agreed price in satoshis
    pub agreed_price_sats: u64,
    /// Contract status
    pub status: ContractStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Completion timestamp
    pub completed_at: Option<DateTime<Utc>>,
}

impl AgentContract {
    /// Create a new contract
    pub fn new(
        id: impl Into<String>,
        hirer: impl Into<String>,
        worker: impl Into<String>,
        task: TaskSpec,
        agreed_price_sats: u64,
    ) -> Self {
        Self {
            id: id.into(),
            hirer: hirer.into(),
            worker: worker.into(),
            task,
            agreed_price_sats,
            status: ContractStatus::Proposed,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Accept the contract
    pub fn accept(&mut self) -> Result<(), String> {
        if self.status != ContractStatus::Proposed {
            return Err(format!("Cannot accept contract in {:?} state", self.status));
        }
        self.status = ContractStatus::Accepted;
        Ok(())
    }

    /// Start work on the contract
    pub fn start_work(&mut self) -> Result<(), String> {
        if self.status != ContractStatus::Accepted {
            return Err(format!(
                "Cannot start work on contract in {:?} state",
                self.status
            ));
        }
        self.status = ContractStatus::InProgress;
        Ok(())
    }

    /// Complete the contract
    pub fn complete(&mut self) -> Result<(), String> {
        if !self.status.is_active() {
            return Err(format!(
                "Cannot complete contract in {:?} state",
                self.status
            ));
        }
        self.status = ContractStatus::Completed;
        self.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Cancel the contract
    pub fn cancel(&mut self) -> Result<(), String> {
        if self.status.is_terminal() {
            return Err(format!("Cannot cancel contract in {:?} state", self.status));
        }
        self.status = ContractStatus::Cancelled;
        Ok(())
    }

    /// Dispute the contract
    pub fn dispute(&mut self) -> Result<(), String> {
        if !self.status.is_active() && self.status != ContractStatus::Completed {
            return Err(format!(
                "Cannot dispute contract in {:?} state",
                self.status
            ));
        }
        self.status = ContractStatus::Disputed;
        Ok(())
    }
}

/// Delegated task within a coordinator pattern
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegatedTask {
    /// Worker agent ID
    pub worker_id: String,
    /// Task specification
    pub task: TaskSpec,
    /// Allocated budget in satoshis
    pub allocated_sats: u64,
    /// Task status
    pub status: ContractStatus,
}

impl DelegatedTask {
    /// Create a new delegated task
    pub fn new(worker_id: impl Into<String>, task: TaskSpec, allocated_sats: u64) -> Self {
        Self {
            worker_id: worker_id.into(),
            task,
            allocated_sats,
            status: ContractStatus::Proposed,
        }
    }

    /// Check if task is complete
    pub fn is_complete(&self) -> bool {
        self.status == ContractStatus::Completed
    }
}

/// Coordinator task that delegates work to multiple agents
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinatorTask {
    /// Coordinator agent ID
    pub coordinator_id: String,
    /// Delegated subtasks
    pub subtasks: Vec<DelegatedTask>,
    /// Coordination fee as percentage (0.0 - 1.0)
    pub coordination_fee_pct: f32,
}

impl CoordinatorTask {
    /// Create a new coordinator task
    pub fn new(coordinator_id: impl Into<String>, coordination_fee_pct: f32) -> Self {
        Self {
            coordinator_id: coordinator_id.into(),
            subtasks: Vec::new(),
            coordination_fee_pct: coordination_fee_pct.clamp(0.0, 1.0),
        }
    }

    /// Add a subtask
    pub fn add_subtask(mut self, task: DelegatedTask) -> Self {
        self.subtasks.push(task);
        self
    }

    /// Calculate total allocated to workers
    pub fn total_worker_allocation(&self) -> u64 {
        self.subtasks.iter().map(|t| t.allocated_sats).sum()
    }

    /// Calculate coordinator fee from total budget
    pub fn coordinator_fee(&self, total_budget_sats: u64) -> u64 {
        (total_budget_sats as f32 * self.coordination_fee_pct) as u64
    }

    /// Calculate total budget needed (worker allocation + coordinator fee)
    pub fn total_budget_needed(&self) -> u64 {
        let worker_total = self.total_worker_allocation();
        // If fee is X%, then worker_total is (1-X)% of total
        // total = worker_total / (1 - fee_pct)
        if self.coordination_fee_pct >= 1.0 {
            return u64::MAX; // Invalid case
        }
        (worker_total as f32 / (1.0 - self.coordination_fee_pct)) as u64
    }

    /// Check if all subtasks are complete
    pub fn all_complete(&self) -> bool {
        !self.subtasks.is_empty() && self.subtasks.iter().all(|t| t.is_complete())
    }

    /// Count completed subtasks
    pub fn completed_count(&self) -> usize {
        self.subtasks.iter().filter(|t| t.is_complete()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_contract_status_is_terminal() {
        assert!(ContractStatus::Completed.is_terminal());
        assert!(ContractStatus::Disputed.is_terminal());
        assert!(ContractStatus::Cancelled.is_terminal());
        assert!(!ContractStatus::Proposed.is_terminal());
        assert!(!ContractStatus::Accepted.is_terminal());
        assert!(!ContractStatus::InProgress.is_terminal());
    }

    #[test]
    fn test_contract_status_is_active() {
        assert!(ContractStatus::Accepted.is_active());
        assert!(ContractStatus::InProgress.is_active());
        assert!(!ContractStatus::Proposed.is_active());
        assert!(!ContractStatus::Completed.is_active());
    }

    #[test]
    fn test_task_spec_builder() {
        let task = TaskSpec::new(
            "code_generation",
            "Generate a function",
            json!({"language": "rust"}),
            "A working function",
        );

        assert_eq!(task.task_type, "code_generation");
        assert_eq!(task.description, "Generate a function");
        assert!(task.deadline.is_none());
    }

    #[test]
    fn test_task_spec_deadline() {
        let future = Utc::now() + chrono::Duration::hours(1);
        let past = Utc::now() - chrono::Duration::hours(1);

        let future_task = TaskSpec::new("test", "desc", json!({}), "output").with_deadline(future);
        let past_task = TaskSpec::new("test", "desc", json!({}), "output").with_deadline(past);

        assert!(!future_task.is_past_deadline());
        assert!(past_task.is_past_deadline());
    }

    #[test]
    fn test_hiring_requirements_builder() {
        let reqs = HiringRequirements::new()
            .with_min_reputation(0.8)
            .require_skill("rust")
            .require_skill("testing")
            .with_max_latency(100);

        assert_eq!(reqs.min_reputation, Some(0.8));
        assert_eq!(reqs.required_skills.len(), 2);
        assert_eq!(reqs.max_latency_ms, Some(100));
    }

    #[test]
    fn test_hire_agent_request() {
        let task = TaskSpec::new("test", "description", json!({}), "output");
        let request = HireAgentRequest::new("agent1", "agent2", task.clone(), 10000);

        assert_eq!(request.hirer_id, "agent1");
        assert_eq!(request.target_agent_id, "agent2");
        assert_eq!(request.budget_sats, 10000);
        assert_eq!(request.task, task);
    }

    #[test]
    fn test_agent_contract_lifecycle() {
        let task = TaskSpec::new("test", "description", json!({}), "output");
        let mut contract = AgentContract::new("c1", "hirer1", "worker1", task, 5000);

        assert_eq!(contract.status, ContractStatus::Proposed);

        // Accept contract
        assert!(contract.accept().is_ok());
        assert_eq!(contract.status, ContractStatus::Accepted);

        // Start work
        assert!(contract.start_work().is_ok());
        assert_eq!(contract.status, ContractStatus::InProgress);

        // Complete
        assert!(contract.complete().is_ok());
        assert_eq!(contract.status, ContractStatus::Completed);
        assert!(contract.completed_at.is_some());
    }

    #[test]
    fn test_agent_contract_invalid_transitions() {
        let task = TaskSpec::new("test", "description", json!({}), "output");
        let mut contract = AgentContract::new("c1", "hirer1", "worker1", task, 5000);

        // Can't start work without accepting
        assert!(contract.start_work().is_err());

        // Accept and complete
        contract.accept().unwrap();
        contract.start_work().unwrap();
        contract.complete().unwrap();

        // Can't accept again after completion
        assert!(contract.accept().is_err());
    }

    #[test]
    fn test_agent_contract_cancel() {
        let task = TaskSpec::new("test", "description", json!({}), "output");
        let mut contract = AgentContract::new("c1", "hirer1", "worker1", task, 5000);

        contract.accept().unwrap();
        assert!(contract.cancel().is_ok());
        assert_eq!(contract.status, ContractStatus::Cancelled);

        // Can't cancel after it's already cancelled
        assert!(contract.cancel().is_err());
    }

    #[test]
    fn test_agent_contract_dispute() {
        let task = TaskSpec::new("test", "description", json!({}), "output");
        let mut contract = AgentContract::new("c1", "hirer1", "worker1", task, 5000);

        contract.accept().unwrap();
        contract.start_work().unwrap();

        assert!(contract.dispute().is_ok());
        assert_eq!(contract.status, ContractStatus::Disputed);
    }

    #[test]
    fn test_delegated_task() {
        let task = TaskSpec::new("subtask", "description", json!({}), "output");
        let delegated = DelegatedTask::new("worker1", task, 2000);

        assert_eq!(delegated.worker_id, "worker1");
        assert_eq!(delegated.allocated_sats, 2000);
        assert_eq!(delegated.status, ContractStatus::Proposed);
        assert!(!delegated.is_complete());
    }

    #[test]
    fn test_coordinator_task_budget() {
        let task1 = TaskSpec::new("task1", "desc", json!({}), "output");
        let task2 = TaskSpec::new("task2", "desc", json!({}), "output");

        let coordinator = CoordinatorTask::new("coordinator1", 0.1)
            .add_subtask(DelegatedTask::new("worker1", task1, 20000))
            .add_subtask(DelegatedTask::new("worker2", task2, 25000));

        // Total worker allocation: 45k sats
        assert_eq!(coordinator.total_worker_allocation(), 45000);

        // If coordinator keeps 10%, total budget needed: 45k / 0.9 = 50k
        let total = coordinator.total_budget_needed();
        assert_eq!(total, 50000);

        // Coordinator fee: 10% of 50k = 5k
        assert_eq!(coordinator.coordinator_fee(total), 5000);
    }

    #[test]
    fn test_coordinator_task_completion() {
        let task = TaskSpec::new("task", "desc", json!({}), "output");
        let mut delegated1 = DelegatedTask::new("worker1", task.clone(), 1000);
        let delegated2 = DelegatedTask::new("worker2", task, 2000);

        let coordinator = CoordinatorTask::new("coord", 0.1)
            .add_subtask(delegated1.clone())
            .add_subtask(delegated2);

        assert!(!coordinator.all_complete());
        assert_eq!(coordinator.completed_count(), 0);

        // Complete first task
        delegated1.status = ContractStatus::Completed;
        let coordinator = CoordinatorTask::new("coord", 0.1)
            .add_subtask(delegated1)
            .add_subtask(DelegatedTask::new(
                "worker2",
                TaskSpec::new("t", "d", json!({}), "o"),
                2000,
            ));

        assert!(!coordinator.all_complete());
        assert_eq!(coordinator.completed_count(), 1);
    }

    #[test]
    fn test_coordinator_fee_clamping() {
        let coordinator = CoordinatorTask::new("coord", 1.5);
        assert_eq!(coordinator.coordination_fee_pct, 1.0);

        let coordinator = CoordinatorTask::new("coord", -0.5);
        assert_eq!(coordinator.coordination_fee_pct, 0.0);
    }

    #[test]
    fn test_contract_serde() {
        let task = TaskSpec::new("test", "description", json!({"key": "value"}), "output");
        let contract = AgentContract::new("c1", "hirer1", "worker1", task, 5000);

        let json = serde_json::to_string(&contract).unwrap();
        let deserialized: AgentContract = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, contract.id);
        assert_eq!(deserialized.agreed_price_sats, contract.agreed_price_sats);
    }

    #[test]
    fn test_contract_status_serde() {
        let status = ContractStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_progress\"");
        let deserialized: ContractStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    #[test]
    fn test_hire_request_serde() {
        let task = TaskSpec::new("test", "desc", json!({}), "output");
        let request = HireAgentRequest::new("agent1", "agent2", task, 10000);

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: HireAgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hirer_id, request.hirer_id);
    }
}
