//! DSPy Lifecycle Signatures.
//!
//! Contains signatures for issue selection, work prioritization, and
//! agent lifecycle state transitions.

use anyhow::Result;
use dsrs::core::signature::MetaSignature;
use dsrs::data::example::Example;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Estimated complexity for an issue or task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Complexity {
    /// Simple task, can be done quickly.
    Low,
    /// Moderate complexity, requires some effort.
    Medium,
    /// Complex task, requires significant effort.
    High,
}

impl std::fmt::Display for Complexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Complexity::Low => write!(f, "LOW"),
            Complexity::Medium => write!(f, "MEDIUM"),
            Complexity::High => write!(f, "HIGH"),
        }
    }
}

impl std::str::FromStr for Complexity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LOW" => Ok(Complexity::Low),
            "MEDIUM" => Ok(Complexity::Medium),
            "HIGH" => Ok(Complexity::High),
            _ => Err(format!("Unknown complexity: {}", s)),
        }
    }
}

/// Agent lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    /// Agent is idle, awaiting work.
    Idle,
    /// Agent is actively working on a task.
    Working,
    /// Agent is blocked, waiting for external input.
    Blocked,
    /// Agent is in provider mode, serving swarm requests.
    Provider,
    /// Agent is shutting down.
    Terminating,
}

impl std::fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleState::Idle => write!(f, "IDLE"),
            LifecycleState::Working => write!(f, "WORKING"),
            LifecycleState::Blocked => write!(f, "BLOCKED"),
            LifecycleState::Provider => write!(f, "PROVIDER"),
            LifecycleState::Terminating => write!(f, "TERMINATING"),
        }
    }
}

impl std::str::FromStr for LifecycleState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "IDLE" => Ok(LifecycleState::Idle),
            "WORKING" => Ok(LifecycleState::Working),
            "BLOCKED" => Ok(LifecycleState::Blocked),
            "PROVIDER" => Ok(LifecycleState::Provider),
            "TERMINATING" => Ok(LifecycleState::Terminating),
            _ => Err(format!("Unknown lifecycle state: {}", s)),
        }
    }
}

/// Issue Selection Signature.
///
/// Choose the best issue to work on from available options.
/// Replaces the simple `state.next_actionable_issue()` logic.
///
/// # Inputs
/// - `available_issues`: JSON array of issues with metadata
/// - `agent_capabilities`: What backends/tools are available
/// - `current_context`: Repository state, recent commits, active branch
///
/// # Outputs
/// - `selected_issue`: Issue number to work on
/// - `rationale`: Why this issue was selected
/// - `estimated_complexity`: LOW, MEDIUM, HIGH
/// - `confidence`: Confidence in selection (0.0-1.0)
#[derive(Debug, Clone)]
pub struct IssueSelectionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for IssueSelectionSignature {
    fn default() -> Self {
        Self {
            instruction: r#"Choose the best issue to work on from the available options.
Consider: dependencies, complexity, urgency, agent capabilities.

Selection criteria:
- Priority labels (P0 > P1 > P2)
- Dependencies (unblocked issues first)
- Complexity vs. agent capabilities
- Recent activity and staleness
- User assignment hints

Output the issue number, your rationale, estimated complexity (LOW/MEDIUM/HIGH),
and confidence in your selection."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl IssueSelectionSignature {
    /// Create a new issue selection signature.
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

impl MetaSignature for IssueSelectionSignature {
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
            "available_issues": {
                "type": "String",
                "desc": "JSON array of available issues with metadata (number, title, labels, priority)",
                "__dsrs_field_type": "input"
            },
            "agent_capabilities": {
                "type": "String",
                "desc": "Agent capabilities: available backends, tools, compute power",
                "__dsrs_field_type": "input"
            },
            "current_context": {
                "type": "String",
                "desc": "Current repository context: branch, recent commits, files changed",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "selected_issue": {
                "type": "String",
                "desc": "Selected issue number as string",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Why this issue was selected over others",
                "__dsrs_field_type": "output"
            },
            "estimated_complexity": {
                "type": "String",
                "desc": "Estimated complexity: LOW, MEDIUM, HIGH",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in selection (0.0-1.0)",
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

/// Work Prioritization Signature.
///
/// Order tasks within an issue or work session by importance and dependencies.
///
/// # Inputs
/// - `task_list`: JSON array of tasks to prioritize
/// - `dependencies`: Task dependency graph as JSON
/// - `deadlines`: Any time constraints or deadlines
///
/// # Outputs
/// - `ordered_tasks`: JSON array of tasks in priority order
/// - `blocking_tasks`: Tasks that are blocking others
/// - `parallel_groups`: Groups of tasks that can run in parallel
#[derive(Debug, Clone)]
pub struct WorkPrioritizationSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for WorkPrioritizationSignature {
    fn default() -> Self {
        Self {
            instruction: r#"Order tasks by importance and dependencies.

Prioritization criteria:
- Dependency graph (blockers first)
- Time constraints and deadlines
- Resource requirements
- Parallelization opportunities
- Risk and complexity

Output:
- ordered_tasks: JSON array of task IDs in priority order
- blocking_tasks: Tasks that block others (should be done first)
- parallel_groups: Groups of tasks that can run simultaneously"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl WorkPrioritizationSignature {
    /// Create a new work prioritization signature.
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

impl MetaSignature for WorkPrioritizationSignature {
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
            "task_list": {
                "type": "String",
                "desc": "JSON array of tasks to prioritize with metadata",
                "__dsrs_field_type": "input"
            },
            "dependencies": {
                "type": "String",
                "desc": "Task dependency graph as JSON (task_id -> [depends_on])",
                "__dsrs_field_type": "input"
            },
            "deadlines": {
                "type": "String",
                "desc": "Any time constraints or deadlines as JSON",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "ordered_tasks": {
                "type": "String",
                "desc": "JSON array of task IDs in priority order",
                "__dsrs_field_type": "output"
            },
            "blocking_tasks": {
                "type": "String",
                "desc": "JSON array of task IDs that are blocking others",
                "__dsrs_field_type": "output"
            },
            "parallel_groups": {
                "type": "String",
                "desc": "JSON array of arrays - groups of tasks that can run in parallel",
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

/// Lifecycle Decision Signature (with Chain-of-Thought).
///
/// Determine agent state transitions. Uses chain-of-thought reasoning
/// to carefully consider state changes and their implications.
///
/// # Inputs
/// - `current_state`: Current lifecycle state (IDLE, WORKING, BLOCKED, etc.)
/// - `recent_events`: Recent events (task completion, errors, user input)
/// - `resource_status`: Memory, CPU, network, wallet balance
///
/// # Outputs
/// - `reasoning`: Chain-of-thought reasoning about the transition
/// - `next_state`: The state to transition to
/// - `transition_reason`: Summary of why this transition
/// - `cleanup_needed`: Any cleanup actions before transition
#[derive(Debug, Clone)]
pub struct LifecycleDecisionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for LifecycleDecisionSignature {
    fn default() -> Self {
        Self {
            instruction: r#"Determine agent state transitions with chain-of-thought reasoning.

State transitions:
- IDLE -> WORKING: Start new task
- IDLE -> PROVIDER: Enter provider mode
- WORKING -> IDLE: Task complete
- WORKING -> BLOCKED: Waiting for input
- BLOCKED -> WORKING: Input received
- * -> TERMINATING: Shutdown requested

Consider:
- Current resource availability
- Pending work queue
- User intent signals
- Error recovery needs

Think through the implications of each possible transition before deciding.
Output your reasoning, the next state, why you chose it, and any cleanup needed."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl LifecycleDecisionSignature {
    /// Create a new lifecycle decision signature.
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

impl MetaSignature for LifecycleDecisionSignature {
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
            "current_state": {
                "type": "String",
                "desc": "Current lifecycle state: IDLE, WORKING, BLOCKED, PROVIDER, TERMINATING",
                "__dsrs_field_type": "input"
            },
            "recent_events": {
                "type": "String",
                "desc": "Recent events as JSON array (task completion, errors, user input)",
                "__dsrs_field_type": "input"
            },
            "resource_status": {
                "type": "String",
                "desc": "Resource status: memory, CPU, network, wallet balance as JSON",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "reasoning": {
                "type": "String",
                "desc": "Chain-of-thought reasoning about the state transition",
                "__dsrs_field_type": "output"
            },
            "next_state": {
                "type": "String",
                "desc": "Next state: IDLE, WORKING, BLOCKED, PROVIDER, TERMINATING",
                "__dsrs_field_type": "output"
            },
            "transition_reason": {
                "type": "String",
                "desc": "Summary reason for state transition",
                "__dsrs_field_type": "output"
            },
            "cleanup_needed": {
                "type": "String",
                "desc": "Cleanup actions needed before transition as JSON array",
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
    fn test_complexity_parse() {
        assert_eq!("LOW".parse::<Complexity>().unwrap(), Complexity::Low);
        assert_eq!("medium".parse::<Complexity>().unwrap(), Complexity::Medium);
        assert_eq!("High".parse::<Complexity>().unwrap(), Complexity::High);
    }

    #[test]
    fn test_complexity_display() {
        assert_eq!(Complexity::Low.to_string(), "LOW");
        assert_eq!(Complexity::Medium.to_string(), "MEDIUM");
        assert_eq!(Complexity::High.to_string(), "HIGH");
    }

    #[test]
    fn test_lifecycle_state_parse() {
        assert_eq!(
            "IDLE".parse::<LifecycleState>().unwrap(),
            LifecycleState::Idle
        );
        assert_eq!(
            "working".parse::<LifecycleState>().unwrap(),
            LifecycleState::Working
        );
        assert_eq!(
            "Blocked".parse::<LifecycleState>().unwrap(),
            LifecycleState::Blocked
        );
        assert_eq!(
            "PROVIDER".parse::<LifecycleState>().unwrap(),
            LifecycleState::Provider
        );
        assert_eq!(
            "terminating".parse::<LifecycleState>().unwrap(),
            LifecycleState::Terminating
        );
    }

    #[test]
    fn test_lifecycle_state_display() {
        assert_eq!(LifecycleState::Idle.to_string(), "IDLE");
        assert_eq!(LifecycleState::Working.to_string(), "WORKING");
        assert_eq!(LifecycleState::Blocked.to_string(), "BLOCKED");
        assert_eq!(LifecycleState::Provider.to_string(), "PROVIDER");
        assert_eq!(LifecycleState::Terminating.to_string(), "TERMINATING");
    }

    #[test]
    fn test_issue_selection_signature() {
        let sig = IssueSelectionSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("available_issues").is_some());
        assert!(inputs.get("agent_capabilities").is_some());
        assert!(inputs.get("current_context").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("selected_issue").is_some());
        assert!(outputs.get("rationale").is_some());
        assert!(outputs.get("estimated_complexity").is_some());
        assert!(outputs.get("confidence").is_some());
    }

    #[test]
    fn test_work_prioritization_signature() {
        let sig = WorkPrioritizationSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("task_list").is_some());
        assert!(inputs.get("dependencies").is_some());
        assert!(inputs.get("deadlines").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("ordered_tasks").is_some());
        assert!(outputs.get("blocking_tasks").is_some());
        assert!(outputs.get("parallel_groups").is_some());
    }

    #[test]
    fn test_lifecycle_decision_signature() {
        let sig = LifecycleDecisionSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("current_state").is_some());
        assert!(inputs.get("recent_events").is_some());
        assert!(inputs.get("resource_status").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("reasoning").is_some());
        assert!(outputs.get("next_state").is_some());
        assert!(outputs.get("transition_reason").is_some());
        assert!(outputs.get("cleanup_needed").is_some());
    }
}
