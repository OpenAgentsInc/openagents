//! DSPy Situation Assessment Signature.
//!
//! Replaces rule-based situation assessment with a learnable DSPy signature.

use anyhow::Result;
use dsrs::core::signature::MetaSignature;
use dsrs::data::example::Example;
use serde_json::{json, Value};

/// Urgency level for recommended actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    /// Must act immediately (e.g., user input, critical error).
    Immediate,
    /// Normal priority, should be handled soon.
    Normal,
    /// Can be deferred or done during idle time.
    Deferred,
}

impl std::fmt::Display for Urgency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Urgency::Immediate => write!(f, "IMMEDIATE"),
            Urgency::Normal => write!(f, "NORMAL"),
            Urgency::Deferred => write!(f, "DEFERRED"),
        }
    }
}

impl std::str::FromStr for Urgency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "IMMEDIATE" => Ok(Urgency::Immediate),
            "NORMAL" => Ok(Urgency::Normal),
            "DEFERRED" => Ok(Urgency::Deferred),
            _ => Err(format!("Unknown urgency: {}", s)),
        }
    }
}

/// Priority action types for OANIX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityAction {
    /// Wait for user direction.
    AwaitUser,
    /// Work on an issue from the repository.
    WorkIssue,
    /// Accept a job from the swarm.
    AcceptJob,
    /// Start provider mode.
    StartProvider,
    /// Initialize identity first.
    InitializeIdentity,
    /// Connect to network.
    ConnectNetwork,
    /// Perform housekeeping (cleanup, sync).
    Housekeeping,
    /// Idle with low-priority background tasks.
    Idle,
}

impl std::fmt::Display for PriorityAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PriorityAction::AwaitUser => write!(f, "AWAIT_USER"),
            PriorityAction::WorkIssue => write!(f, "WORK_ISSUE"),
            PriorityAction::AcceptJob => write!(f, "ACCEPT_JOB"),
            PriorityAction::StartProvider => write!(f, "START_PROVIDER"),
            PriorityAction::InitializeIdentity => write!(f, "INITIALIZE_IDENTITY"),
            PriorityAction::ConnectNetwork => write!(f, "CONNECT_NETWORK"),
            PriorityAction::Housekeeping => write!(f, "HOUSEKEEPING"),
            PriorityAction::Idle => write!(f, "IDLE"),
        }
    }
}

impl std::str::FromStr for PriorityAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "AWAIT_USER" => Ok(PriorityAction::AwaitUser),
            "WORK_ISSUE" => Ok(PriorityAction::WorkIssue),
            "ACCEPT_JOB" => Ok(PriorityAction::AcceptJob),
            "START_PROVIDER" => Ok(PriorityAction::StartProvider),
            "INITIALIZE_IDENTITY" => Ok(PriorityAction::InitializeIdentity),
            "CONNECT_NETWORK" => Ok(PriorityAction::ConnectNetwork),
            "HOUSEKEEPING" => Ok(PriorityAction::Housekeeping),
            "IDLE" => Ok(PriorityAction::Idle),
            _ => Err(format!("Unknown priority action: {}", s)),
        }
    }
}

/// Situation Assessment Signature.
///
/// Analyzes the current system state and determines what the agent should
/// prioritize. This replaces the rule-based `SituationAssessment::from_manifest()`
/// with a learnable approach.
///
/// # Inputs
/// - `system_state`: Current hardware/compute state as JSON (from OanixManifest)
/// - `pending_events`: Pending events and requests in queue
/// - `recent_history`: Recent decisions and their outcomes
///
/// # Outputs
/// - `priority_action`: What to do next (AWAIT_USER, WORK_ISSUE, etc.)
/// - `urgency`: Urgency level (IMMEDIATE, NORMAL, DEFERRED)
/// - `reasoning`: Explanation for the recommended action
/// - `confidence`: Confidence in this assessment (0.0-1.0)
#[derive(Debug, Clone)]
pub struct SituationAssessmentSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for SituationAssessmentSignature {
    fn default() -> Self {
        Self {
            instruction: r#"Analyze current system state and determine what the agent should prioritize.
Consider: available compute, network state, pending work, user intent.

Priority actions (choose one):
- AWAIT_USER: Wait for user direction (default when idle)
- WORK_ISSUE: Work on an issue from the repository
- ACCEPT_JOB: Accept a job from the NIP-90 swarm
- START_PROVIDER: Begin provider mode to earn sats
- INITIALIZE_IDENTITY: Set up Nostr identity first
- CONNECT_NETWORK: Establish network connectivity
- HOUSEKEEPING: Cleanup, sync state, refresh manifest
- IDLE: Low-priority background mode

Urgency levels:
- IMMEDIATE: Must act now (user input, critical error)
- NORMAL: Should be handled soon
- DEFERRED: Can wait, do during idle time

Assess the situation and recommend the most appropriate action with reasoning."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl SituationAssessmentSignature {
    /// Create a new situation assessment signature.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom instruction.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    /// Add a demonstration example.
    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for SituationAssessmentSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "system_state": {
                "type": "String",
                "desc": "Current hardware/compute state as JSON",
                "__dsrs_field_type": "input"
            },
            "pending_events": {
                "type": "String",
                "desc": "Pending events and requests in queue as JSON array",
                "__dsrs_field_type": "input"
            },
            "recent_history": {
                "type": "String",
                "desc": "Recent decisions and their outcomes as JSON array",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "priority_action": {
                "type": "String",
                "desc": "Priority action: AWAIT_USER, WORK_ISSUE, ACCEPT_JOB, START_PROVIDER, INITIALIZE_IDENTITY, CONNECT_NETWORK, HOUSEKEEPING, IDLE",
                "__dsrs_field_type": "output"
            },
            "urgency": {
                "type": "String",
                "desc": "Urgency level: IMMEDIATE, NORMAL, DEFERRED",
                "__dsrs_field_type": "output"
            },
            "reasoning": {
                "type": "String",
                "desc": "Explanation for the recommended action",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in this assessment (0.0-1.0)",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urgency_parse() {
        assert_eq!("IMMEDIATE".parse::<Urgency>().unwrap(), Urgency::Immediate);
        assert_eq!("normal".parse::<Urgency>().unwrap(), Urgency::Normal);
        assert_eq!("Deferred".parse::<Urgency>().unwrap(), Urgency::Deferred);
    }

    #[test]
    fn test_priority_action_parse() {
        assert_eq!(
            "AWAIT_USER".parse::<PriorityAction>().unwrap(),
            PriorityAction::AwaitUser
        );
        assert_eq!(
            "work_issue".parse::<PriorityAction>().unwrap(),
            PriorityAction::WorkIssue
        );
        assert_eq!(
            "Accept_Job".parse::<PriorityAction>().unwrap(),
            PriorityAction::AcceptJob
        );
    }

    #[test]
    fn test_urgency_display() {
        assert_eq!(Urgency::Immediate.to_string(), "IMMEDIATE");
        assert_eq!(Urgency::Normal.to_string(), "NORMAL");
        assert_eq!(Urgency::Deferred.to_string(), "DEFERRED");
    }

    #[test]
    fn test_priority_action_display() {
        assert_eq!(PriorityAction::AwaitUser.to_string(), "AWAIT_USER");
        assert_eq!(PriorityAction::WorkIssue.to_string(), "WORK_ISSUE");
        assert_eq!(PriorityAction::StartProvider.to_string(), "START_PROVIDER");
    }

    #[test]
    fn test_signature_fields() {
        let sig = SituationAssessmentSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("system_state").is_some());
        assert!(inputs.get("pending_events").is_some());
        assert!(inputs.get("recent_history").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("priority_action").is_some());
        assert!(outputs.get("urgency").is_some());
        assert!(outputs.get("reasoning").is_some());
        assert!(outputs.get("confidence").is_some());
    }
}
