//! Agent governance and oversight levels
//!
//! Implements autonomy levels, human oversight controls, action limits,
//! approval requirements, and escalation triggers for agent governance.

use serde::{Deserialize, Serialize};

/// Action type that can be performed by an agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Request inference from a model
    InferenceRequest,
    /// Install a skill
    SkillInstall,
    /// Join a coalition
    CoalitionJoin,
    /// Hire another agent
    HireAgent,
    /// Spawn a new agent
    SpawnAgent,
    /// Send payment
    PaymentSend,
    /// Contribute data
    DataContribute,
}

impl ActionType {
    /// Get action type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::InferenceRequest => "inference_request",
            ActionType::SkillInstall => "skill_install",
            ActionType::CoalitionJoin => "coalition_join",
            ActionType::HireAgent => "hire_agent",
            ActionType::SpawnAgent => "spawn_agent",
            ActionType::PaymentSend => "payment_send",
            ActionType::DataContribute => "data_contribute",
        }
    }
}

/// Escalation condition that can trigger oversight
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationCondition {
    /// Spend exceeds configured limit
    SpendExceedsLimit,
    /// Reputation drops below threshold
    ReputationDrops(f32),
    /// A dispute was filed against the agent
    DisputeFiled,
    /// Unusual activity pattern detected
    UnusualActivity,
    /// Agent balance is critically low
    LowBalance,
}

/// Action to take when escalation condition is met
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationAction {
    /// Send notification to human overseer
    NotifyHuman,
    /// Pause agent operations
    PauseAgent,
    /// Require human approval for next actions
    RequireApproval,
    /// Terminate the agent
    TerminateAgent,
}

impl EscalationAction {
    /// Check if action is terminal (ends agent operation)
    pub fn is_terminal(&self) -> bool {
        matches!(self, EscalationAction::TerminateAgent)
    }

    /// Check if action blocks further operations
    pub fn blocks_operations(&self) -> bool {
        matches!(
            self,
            EscalationAction::PauseAgent
                | EscalationAction::RequireApproval
                | EscalationAction::TerminateAgent
        )
    }
}

/// Action limits for agent operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionLimits {
    /// Maximum spend per single action in satoshis
    pub max_spend_per_action_sats: u64,
    /// Maximum spend per day in satoshis
    pub max_spend_per_day_sats: u64,
    /// Maximum coalition size agent can join
    pub max_coalition_size: u32,
    /// Allowed action types (empty = all allowed)
    pub allowed_action_types: Vec<ActionType>,
    /// Blocked action types (takes precedence over allowed)
    pub blocked_action_types: Vec<ActionType>,
}

impl ActionLimits {
    /// Create new action limits
    pub fn new(max_spend_per_action_sats: u64, max_spend_per_day_sats: u64) -> Self {
        Self {
            max_spend_per_action_sats,
            max_spend_per_day_sats,
            max_coalition_size: 10,
            allowed_action_types: Vec::new(),
            blocked_action_types: Vec::new(),
        }
    }

    /// Set maximum coalition size
    pub fn with_max_coalition_size(mut self, max: u32) -> Self {
        self.max_coalition_size = max;
        self
    }

    /// Allow specific action type
    pub fn allow_action(mut self, action: ActionType) -> Self {
        if !self.allowed_action_types.contains(&action) {
            self.allowed_action_types.push(action);
        }
        self
    }

    /// Block specific action type
    pub fn block_action(mut self, action: ActionType) -> Self {
        if !self.blocked_action_types.contains(&action) {
            self.blocked_action_types.push(action);
        }
        self
    }

    /// Check if action type is allowed
    pub fn is_action_allowed(&self, action: &ActionType) -> bool {
        // Blocked takes precedence
        if self.blocked_action_types.contains(action) {
            return false;
        }

        // If allowed list is empty, everything is allowed (except blocked)
        if self.allowed_action_types.is_empty() {
            return true;
        }

        // Otherwise must be in allowed list
        self.allowed_action_types.contains(action)
    }

    /// Check if spend amount is within limits
    pub fn is_spend_allowed(&self, amount_sats: u64) -> bool {
        amount_sats <= self.max_spend_per_action_sats
    }
}

/// Approval requirement for specific actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequirement {
    /// Action type requiring approval
    pub action_type: ActionType,
    /// Threshold above which approval is required (None = always required)
    pub threshold_sats: Option<u64>,
    /// Whether human approval is specifically required
    pub requires_human: bool,
}

impl ApprovalRequirement {
    /// Create new approval requirement
    pub fn new(action_type: ActionType) -> Self {
        Self {
            action_type,
            threshold_sats: None,
            requires_human: true,
        }
    }

    /// Set threshold for approval
    pub fn with_threshold(mut self, threshold_sats: u64) -> Self {
        self.threshold_sats = Some(threshold_sats);
        self
    }

    /// Set whether human approval is required
    pub fn requires_human(mut self, required: bool) -> Self {
        self.requires_human = required;
        self
    }

    /// Check if action requires approval given the amount
    pub fn needs_approval(&self, action: &ActionType, amount_sats: Option<u64>) -> bool {
        if action != &self.action_type {
            return false;
        }

        if let Some(threshold) = self.threshold_sats {
            if let Some(amount) = amount_sats {
                return amount > threshold;
            }
        }

        // No threshold means always required, or no amount provided
        true
    }
}

/// Escalation trigger configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscalationTrigger {
    /// Condition that triggers escalation
    pub condition: EscalationCondition,
    /// Action to take when triggered
    pub action: EscalationAction,
}

impl EscalationTrigger {
    /// Create new escalation trigger
    pub fn new(condition: EscalationCondition, action: EscalationAction) -> Self {
        Self { condition, action }
    }
}

/// Sponsor control permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SponsorControls {
    /// Can pause agent operations
    pub can_pause: bool,
    /// Can terminate the agent
    pub can_terminate: bool,
    /// Can modify autonomy policy
    pub can_modify_policy: bool,
    /// Can withdraw funds from agent wallet
    pub can_withdraw_funds: bool,
    /// Percentage of agent earnings that go to sponsor (0.0 - 1.0)
    pub receives_earnings_pct: f32,
}

impl SponsorControls {
    /// Create new sponsor controls with full permissions
    pub fn full_control() -> Self {
        Self {
            can_pause: true,
            can_terminate: true,
            can_modify_policy: true,
            can_withdraw_funds: true,
            receives_earnings_pct: 0.0,
        }
    }

    /// Create sponsor controls with minimal permissions
    pub fn minimal() -> Self {
        Self {
            can_pause: false,
            can_terminate: false,
            can_modify_policy: false,
            can_withdraw_funds: false,
            receives_earnings_pct: 0.0,
        }
    }

    /// Set earnings percentage
    pub fn with_earnings_pct(mut self, pct: f32) -> Self {
        self.receives_earnings_pct = pct.clamp(0.0, 1.0);
        self
    }

    /// Set pause permission
    pub fn can_pause(mut self, can: bool) -> Self {
        self.can_pause = can;
        self
    }

    /// Set terminate permission
    pub fn can_terminate(mut self, can: bool) -> Self {
        self.can_terminate = can;
        self
    }

    /// Set policy modification permission
    pub fn can_modify_policy(mut self, can: bool) -> Self {
        self.can_modify_policy = can;
        self
    }

    /// Set fund withdrawal permission
    pub fn can_withdraw_funds(mut self, can: bool) -> Self {
        self.can_withdraw_funds = can;
        self
    }
}

impl Default for SponsorControls {
    fn default() -> Self {
        Self::full_control()
    }
}

/// Autonomy policy for agent governance
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomyPolicy {
    /// Autonomy level
    pub level: crate::agent_lifecycle::AutonomyLevel,
    /// Action limits
    pub action_limits: ActionLimits,
    /// Approval requirements
    pub approval_requirements: Vec<ApprovalRequirement>,
    /// Escalation triggers
    pub escalation_triggers: Vec<EscalationTrigger>,
}

impl AutonomyPolicy {
    /// Create new autonomy policy
    pub fn new(level: crate::agent_lifecycle::AutonomyLevel, action_limits: ActionLimits) -> Self {
        Self {
            level,
            action_limits,
            approval_requirements: Vec::new(),
            escalation_triggers: Vec::new(),
        }
    }

    /// Add approval requirement
    pub fn add_approval(mut self, requirement: ApprovalRequirement) -> Self {
        self.approval_requirements.push(requirement);
        self
    }

    /// Add escalation trigger
    pub fn add_escalation(mut self, trigger: EscalationTrigger) -> Self {
        self.escalation_triggers.push(trigger);
        self
    }

    /// Check if action requires approval
    pub fn requires_approval(&self, action: &ActionType, amount_sats: Option<u64>) -> bool {
        self.approval_requirements
            .iter()
            .any(|req| req.needs_approval(action, amount_sats))
    }

    /// Check if action is allowed by policy
    pub fn is_action_allowed(&self, action: &ActionType, amount_sats: Option<u64>) -> bool {
        // Check action limits
        if !self.action_limits.is_action_allowed(action) {
            return false;
        }

        // Check spend limits if amount provided
        if let Some(amount) = amount_sats {
            if !self.action_limits.is_spend_allowed(amount) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_lifecycle::AutonomyLevel;

    #[test]
    fn test_action_type_as_str() {
        assert_eq!(ActionType::InferenceRequest.as_str(), "inference_request");
        assert_eq!(ActionType::HireAgent.as_str(), "hire_agent");
        assert_eq!(ActionType::SpawnAgent.as_str(), "spawn_agent");
    }

    #[test]
    fn test_escalation_action_is_terminal() {
        assert!(EscalationAction::TerminateAgent.is_terminal());
        assert!(!EscalationAction::NotifyHuman.is_terminal());
        assert!(!EscalationAction::PauseAgent.is_terminal());
    }

    #[test]
    fn test_escalation_action_blocks_operations() {
        assert!(EscalationAction::PauseAgent.blocks_operations());
        assert!(EscalationAction::RequireApproval.blocks_operations());
        assert!(EscalationAction::TerminateAgent.blocks_operations());
        assert!(!EscalationAction::NotifyHuman.blocks_operations());
    }

    #[test]
    fn test_action_limits_builder() {
        let limits = ActionLimits::new(10000, 100000)
            .with_max_coalition_size(5)
            .allow_action(ActionType::InferenceRequest)
            .block_action(ActionType::SpawnAgent);

        assert_eq!(limits.max_spend_per_action_sats, 10000);
        assert_eq!(limits.max_spend_per_day_sats, 100000);
        assert_eq!(limits.max_coalition_size, 5);
    }

    #[test]
    fn test_action_limits_is_action_allowed() {
        let limits = ActionLimits::new(10000, 100000)
            .allow_action(ActionType::InferenceRequest)
            .allow_action(ActionType::HireAgent);

        // Allowed
        assert!(limits.is_action_allowed(&ActionType::InferenceRequest));
        assert!(limits.is_action_allowed(&ActionType::HireAgent));

        // Not in allowed list
        assert!(!limits.is_action_allowed(&ActionType::SpawnAgent));
    }

    #[test]
    fn test_action_limits_blocked_takes_precedence() {
        let limits = ActionLimits::new(10000, 100000)
            .allow_action(ActionType::InferenceRequest)
            .block_action(ActionType::InferenceRequest);

        // Blocked takes precedence over allowed
        assert!(!limits.is_action_allowed(&ActionType::InferenceRequest));
    }

    #[test]
    fn test_action_limits_empty_allowed_means_all() {
        let limits = ActionLimits::new(10000, 100000);

        // Empty allowed list means all actions are allowed
        assert!(limits.is_action_allowed(&ActionType::InferenceRequest));
        assert!(limits.is_action_allowed(&ActionType::SpawnAgent));
    }

    #[test]
    fn test_action_limits_is_spend_allowed() {
        let limits = ActionLimits::new(10000, 100000);

        assert!(limits.is_spend_allowed(5000));
        assert!(limits.is_spend_allowed(10000));
        assert!(!limits.is_spend_allowed(10001));
    }

    #[test]
    fn test_approval_requirement_builder() {
        let req = ApprovalRequirement::new(ActionType::PaymentSend)
            .with_threshold(5000)
            .requires_human(true);

        assert_eq!(req.action_type, ActionType::PaymentSend);
        assert_eq!(req.threshold_sats, Some(5000));
        assert!(req.requires_human);
    }

    #[test]
    fn test_approval_requirement_needs_approval() {
        let req = ApprovalRequirement::new(ActionType::PaymentSend).with_threshold(5000);

        // Below threshold
        assert!(!req.needs_approval(&ActionType::PaymentSend, Some(4000)));

        // Above threshold
        assert!(req.needs_approval(&ActionType::PaymentSend, Some(6000)));

        // Different action
        assert!(!req.needs_approval(&ActionType::InferenceRequest, Some(6000)));

        // No amount provided
        assert!(req.needs_approval(&ActionType::PaymentSend, None));
    }

    #[test]
    fn test_approval_requirement_no_threshold() {
        let req = ApprovalRequirement::new(ActionType::SpawnAgent);

        // Always requires approval without threshold
        assert!(req.needs_approval(&ActionType::SpawnAgent, Some(1000)));
        assert!(req.needs_approval(&ActionType::SpawnAgent, None));
    }

    #[test]
    fn test_escalation_trigger() {
        let trigger = EscalationTrigger::new(
            EscalationCondition::SpendExceedsLimit,
            EscalationAction::PauseAgent,
        );

        assert_eq!(trigger.condition, EscalationCondition::SpendExceedsLimit);
        assert_eq!(trigger.action, EscalationAction::PauseAgent);
    }

    #[test]
    fn test_sponsor_controls_full() {
        let controls = SponsorControls::full_control();

        assert!(controls.can_pause);
        assert!(controls.can_terminate);
        assert!(controls.can_modify_policy);
        assert!(controls.can_withdraw_funds);
        assert_eq!(controls.receives_earnings_pct, 0.0);
    }

    #[test]
    fn test_sponsor_controls_minimal() {
        let controls = SponsorControls::minimal();

        assert!(!controls.can_pause);
        assert!(!controls.can_terminate);
        assert!(!controls.can_modify_policy);
        assert!(!controls.can_withdraw_funds);
    }

    #[test]
    fn test_sponsor_controls_builder() {
        let controls = SponsorControls::minimal()
            .can_pause(true)
            .with_earnings_pct(0.15);

        assert!(controls.can_pause);
        assert!(!controls.can_terminate);
        assert_eq!(controls.receives_earnings_pct, 0.15);
    }

    #[test]
    fn test_sponsor_controls_earnings_clamping() {
        let controls = SponsorControls::minimal().with_earnings_pct(1.5);
        assert_eq!(controls.receives_earnings_pct, 1.0);

        let controls = SponsorControls::minimal().with_earnings_pct(-0.5);
        assert_eq!(controls.receives_earnings_pct, 0.0);
    }

    #[test]
    fn test_autonomy_policy() {
        let limits = ActionLimits::new(10000, 100000);
        let policy = AutonomyPolicy::new(AutonomyLevel::Supervised, limits)
            .add_approval(ApprovalRequirement::new(ActionType::PaymentSend).with_threshold(5000))
            .add_escalation(EscalationTrigger::new(
                EscalationCondition::SpendExceedsLimit,
                EscalationAction::NotifyHuman,
            ));

        assert_eq!(policy.level, AutonomyLevel::Supervised);
        assert_eq!(policy.approval_requirements.len(), 1);
        assert_eq!(policy.escalation_triggers.len(), 1);
    }

    #[test]
    fn test_autonomy_policy_requires_approval() {
        let limits = ActionLimits::new(10000, 100000);
        let policy = AutonomyPolicy::new(AutonomyLevel::Supervised, limits)
            .add_approval(ApprovalRequirement::new(ActionType::PaymentSend).with_threshold(5000));

        // Below threshold - no approval needed
        assert!(!policy.requires_approval(&ActionType::PaymentSend, Some(4000)));

        // Above threshold - approval needed
        assert!(policy.requires_approval(&ActionType::PaymentSend, Some(6000)));
    }

    #[test]
    fn test_autonomy_policy_is_action_allowed() {
        let limits = ActionLimits::new(10000, 100000)
            .allow_action(ActionType::InferenceRequest)
            .block_action(ActionType::SpawnAgent);

        let policy = AutonomyPolicy::new(AutonomyLevel::Autonomous, limits);

        // Allowed action
        assert!(policy.is_action_allowed(&ActionType::InferenceRequest, Some(5000)));

        // Blocked action
        assert!(!policy.is_action_allowed(&ActionType::SpawnAgent, Some(5000)));

        // Spend exceeds limit
        assert!(!policy.is_action_allowed(&ActionType::InferenceRequest, Some(15000)));
    }

    #[test]
    fn test_escalation_condition_serde() {
        let condition = EscalationCondition::ReputationDrops(0.5);
        let json = serde_json::to_string(&condition).unwrap();
        let deserialized: EscalationCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, condition);
    }

    #[test]
    fn test_action_type_serde() {
        let action = ActionType::HireAgent;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"hire_agent\"");
        let deserialized: ActionType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, action);
    }

    #[test]
    fn test_sponsor_controls_serde() {
        let controls = SponsorControls::full_control().with_earnings_pct(0.2);
        let json = serde_json::to_string(&controls).unwrap();
        let deserialized: SponsorControls = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.receives_earnings_pct, 0.2);
    }
}
